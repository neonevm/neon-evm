use std::ops::{Deref, Range};

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

#[cfg_attr(test, derive(Debug, PartialEq))]
enum Inner {
    Owned(Vec<u8>),
    Account {
        key: Pubkey,
        range: Range<usize>,
        data: *const u8,
    },
    AccountUninit {
        key: Pubkey,
        range: Range<usize>,
    },
}

#[cfg_attr(test, derive(Debug))]
pub struct Buffer {
    // We maintain a ptr and len to be able to construct a slice without having to discriminate
    // inner. This means we should not allow mutation of inner after the construction of a buffer.
    ptr: *const u8,
    len: usize,
    inner: Inner,
}

#[cfg(test)]
impl core::cmp::PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Buffer {
    fn new(inner: Inner) -> Self {
        let (ptr, len) = match &inner {
            Inner::Owned(data) => (data.as_ptr(), data.len()),
            Inner::Account { data, range, .. } => {
                let ptr = unsafe { data.add(range.start) };
                (ptr, range.len())
            }
            Inner::AccountUninit { .. } => (std::ptr::null(), 0),
        };

        Buffer { ptr, len, inner }
    }

    /// # Safety
    ///
    /// This function was marked as unsafe until correct lifetimes will be set.
    /// At the moment, `Buffer` may outlive `account`, since no lifetimes has been set,
    /// so they are not checked by the compiler and it's the user's responsibility to take
    /// care of them.
    #[must_use]
    pub unsafe fn from_account(account: &AccountInfo, range: Range<usize>) -> Self {
        let data = unsafe {
            // todo cell_leak #69099
            let ptr = account.data.as_ptr();
            (*ptr).as_ptr()
        };

        Buffer::new(Inner::Account {
            key: *account.key,
            data,
            range,
        })
    }

    #[must_use]
    pub fn from_vec(v: Vec<u8>) -> Self {
        Self::new(Inner::Owned(v))
    }

    #[must_use]
    pub fn from_slice(v: &[u8]) -> Self {
        Self::from_vec(v.to_vec())
    }

    #[must_use]
    pub fn empty() -> Self {
        Buffer::new(Inner::Owned(Vec::default()))
    }

    #[must_use]
    pub fn is_initialized(&self) -> bool {
        !matches!(self.inner, Inner::AccountUninit { .. })
    }

    #[must_use]
    pub fn uninit_data(&self) -> Option<(Pubkey, Range<usize>)> {
        if let Inner::AccountUninit { key, range } = &self.inner {
            Some((*key, range.clone()))
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub fn get_or_default(&self, index: usize) -> u8 {
        debug_assert!(!self.ptr.is_null());

        if index < self.len {
            unsafe { self.ptr.add(index).read() }
        } else {
            0
        }
    }
}

impl Deref for Buffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        debug_assert!(!self.ptr.is_null());

        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Clone for Buffer {
    #[inline]
    fn clone(&self) -> Self {
        match &self.inner {
            Inner::Owned { .. } => Self::from_slice(self),
            Inner::Account { key, data, range } => Self::new(Inner::Account {
                key: *key,
                range: range.clone(),
                data: *data,
            }),
            Inner::AccountUninit { key, range } => Self::new(Inner::AccountUninit {
                key: *key,
                range: range.clone(),
            }),
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::empty()
    }
}

impl serde::Serialize for Buffer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStructVariant;

        match &self.inner {
            Inner::Owned(data) => {
                let bytes = serde_bytes::Bytes::new(data);
                serializer.serialize_newtype_variant("evm_buffer", 1, "owned", bytes)
            }
            Inner::Account { key, range, .. } => {
                let mut sv = serializer.serialize_struct_variant("evm_buffer", 2, "account", 2)?;
                sv.serialize_field("key", key)?;
                sv.serialize_field("range", range)?;
                sv.end()
            }
            Inner::AccountUninit { .. } => {
                unreachable!()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for Buffer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BufferVisitor;

        impl<'de> serde::de::Visitor<'de> for BufferVisitor {
            type Value = Buffer;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("EVM Buffer")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Buffer::empty())
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Buffer::from_slice(v))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let key = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let range = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok(Buffer::new(Inner::AccountUninit { key, range }))
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::EnumAccess<'de>,
            {
                use serde::de::VariantAccess;

                let (index, variant) = data.variant::<u32>()?;
                match index {
                    0 => variant.unit_variant().map(|()| Buffer::empty()),
                    1 => variant.newtype_variant().map(Buffer::from_slice),
                    2 => variant.struct_variant(&["key", "range"], self),
                    _ => Err(serde::de::Error::unknown_variant(
                        "_",
                        &["owned", "account"],
                    )),
                }
            }
        }

        deserializer.deserialize_enum("evm_buffer", &["owned", "account"], BufferVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{executor::OwnedAccountInfo, types::vector::into_vector};
    use solana_program::account_info::IntoAccountInfo;

    macro_rules! assert_slice_ptr_eq {
        ($actual:expr, $expected:expr) => {{
            let actual: &[_] = $actual;
            let (expected_ptr, expected_len): (*const _, usize) = $expected;
            assert_eq!(actual.as_ptr(), expected_ptr);
            assert_eq!(actual.len(), expected_len);
        }};
    }

    #[test]
    fn test_deref_owned_empty() {
        let data = Vec::default();
        let expected = (data.as_ptr(), data.len());
        assert_slice_ptr_eq!(&*Buffer::default(), expected);
    }

    #[test]
    fn test_deref_owned_non_empty() {
        let data = vec![1];
        let expected = (data.as_ptr(), data.len());
        assert_slice_ptr_eq!(&*Buffer::from_vec(data), expected);
    }

    impl OwnedAccountInfo {
        fn with_data(data: Vec<u8>) -> Self {
            OwnedAccountInfo {
                key: Pubkey::default(),
                lamports: 0,
                data: into_vector(data),
                owner: Pubkey::default(),
                rent_epoch: 0,
                is_signer: false,
                is_writable: false,
                executable: false,
            }
        }
    }

    #[test]
    fn test_deref_account_empty() {
        let data = Vec::default();
        let expected = (data.as_ptr(), data.len());
        let mut account_info = OwnedAccountInfo::with_data(data);
        assert_slice_ptr_eq!(
            &*unsafe { Buffer::from_account(&account_info.into_account_info(), 0..expected.1) },
            expected
        );
    }

    #[test]
    fn test_deref_account_non_empty() {
        let data = vec![1];
        let expected = (data.as_ptr(), data.len());
        let mut account_info = OwnedAccountInfo::with_data(data);
        assert_slice_ptr_eq!(
            &*unsafe { Buffer::from_account(&account_info.into_account_info(), 0..expected.1) },
            expected
        );
    }

    #[test]
    #[should_panic(expected = "assertion failed: !self.ptr.is_null()")]
    fn test_deref_account_uninit() {
        let _: &[u8] = &Buffer::new(Inner::AccountUninit {
            key: Pubkey::default(),
            range: 0..0,
        });
    }

    #[test]
    fn historic_empty_deserialization_works() {
        let serialized = [
            0, 0, 0, 0, // Variant
        ];
        let deserialized = Buffer::empty();
        assert_eq!(
            bincode::deserialize::<Buffer>(&serialized).unwrap(),
            deserialized
        );
    }

    #[test]
    fn non_empty_owned_serialization_works() {
        let deserialized = Buffer::from_vec(vec![0xcc; 3]);
        let serialized = [
            1, 0, 0, 0, // Variant
            3, 0, 0, 0, 0, 0, 0, 0, // Byte count
            0xcc, 0xcc, 0xcc, // Bytes
        ];
        assert_eq!(bincode::serialize(&deserialized).unwrap(), serialized);
    }

    #[test]
    fn non_empty_owned_deserialization_works() {
        let serialized = [
            1, 0, 0, 0, // Variant
            3, 0, 0, 0, 0, 0, 0, 0, // Byte count
            0xcc, 0xcc, 0xcc, // Bytes
        ];
        let deserialized = Buffer::from_vec(vec![0xcc; 3]);
        assert_eq!(
            bincode::deserialize::<Buffer>(&serialized).unwrap(),
            deserialized
        );
    }

    #[test]
    fn non_empty_account_serialization_works() {
        let mut account = OwnedAccountInfo {
            key: Pubkey::from([0xaa; 32]),
            is_signer: false,
            is_writable: false,
            lamports: 0,
            data: vec![0xcc; 10],
            owner: Pubkey::from([0xbb; 32]),
            executable: false,
            rent_epoch: 0,
        };
        let deserialized = unsafe { Buffer::from_account(&account.into_account_info(), 6..8) };
        let serialized = [
            2, 0, 0, 0, // Variant
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, // Pubkey
            6, 0, 0, 0, 0, 0, 0, 0, // Range start
            8, 0, 0, 0, 0, 0, 0, 0, // Range end
        ];
        assert_eq!(bincode::serialize(&deserialized).unwrap(), serialized);
    }

    #[test]
    fn non_empty_account_deserialization_works() {
        let serialized = [
            2, 0, 0, 0, // Variant
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, // Pubkey
            6, 0, 0, 0, 0, 0, 0, 0, // Range start
            8, 0, 0, 0, 0, 0, 0, 0, // Range end
        ];
        let deserialized = Buffer::new(Inner::AccountUninit {
            key: Pubkey::from([0xaa; 32]),
            range: 6..8,
        });
        assert_eq!(
            bincode::deserialize::<Buffer>(&serialized).unwrap(),
            deserialized
        );
    }

    #[test]
    #[should_panic(expected = "unreachable")]
    fn account_uninit_serialization_fails() {
        let _: Vec<u8> = bincode::serialize(&Buffer::new(Inner::AccountUninit {
            key: Pubkey::default(),
            range: 0..0,
        }))
        .unwrap();
    }
}
