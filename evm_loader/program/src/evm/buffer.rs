use std::{
    alloc::{GlobalAlloc, Layout},
    ops::{Deref, Range},
    ptr::NonNull,
};

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

const BUFFER_ALIGN: usize = 1;

#[derive(Debug)]
enum Inner {
    Empty,
    Owned {
        ptr: NonNull<u8>,
        len: usize,
    },
    Account {
        key: Pubkey,
        data: *mut u8,
        range: Range<usize>,
    },
    AccountUninit {
        key: Pubkey,
        range: Range<usize>,
    },
}

#[derive(Debug)]
pub struct Buffer {
    ptr: *const u8,
    len: usize,
    inner: Inner,
}

impl Buffer {
    fn new(inner: Inner) -> Self {
        let (ptr, len) = match &inner {
            Inner::Empty => (NonNull::dangling().as_ptr(), 0),
            Inner::Owned { ptr, len } => (ptr.as_ptr(), *len),
            Inner::Account { data, range, .. } => {
                let ptr = unsafe { data.add(range.start) };
                (ptr, range.len())
            }
            Inner::AccountUninit { .. } => (std::ptr::null_mut(), 0),
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
        // todo cell_leak #69099
        let ptr = account.data.as_ptr();
        let data = (*ptr).as_mut_ptr();

        Buffer::new(Inner::Account {
            key: *account.key,
            data,
            range,
        })
    }

    #[must_use]
    pub fn from_slice(v: &[u8]) -> Self {
        if v.is_empty() {
            return Self::empty();
        }

        unsafe {
            let len = v.len();

            let layout = Layout::from_size_align_unchecked(len, BUFFER_ALIGN);
            let ptr = crate::allocator::EVM.alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            cfg_if::cfg_if! {
                if #[cfg(target_os = "solana")] {
                    solana_program::syscalls::sol_memcpy_(ptr, v.as_ptr(), len as u64);
                } else {
                    std::ptr::copy_nonoverlapping(v.as_ptr(), ptr, len);
                }
            }

            Buffer::new(Inner::Owned {
                ptr: NonNull::new_unchecked(ptr),
                len,
            })
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Buffer::new(Inner::Empty)
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

    #[inline]
    #[must_use]
    pub fn get_unchecked_at(&self, index: usize) -> u8 {
        unsafe { self.ptr.add(index).read() }
    }

    #[inline]
    #[must_use]
    pub fn get_u16_or_default(&self, index: usize) -> u16 {
        if self.len() < index + 2 {
            return u16::default();
        };

        u16::from_be_bytes(*arrayref::array_ref![*self, index, 2])
    }

    #[inline]
    #[must_use]
    pub fn get_i16_or_default(&self, index: usize) -> i16 {
        if self.len() < index + 2 {
            return i16::default();
        };

        i16::from_be_bytes(*arrayref::array_ref![*self, index, 2])
    }
}

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(&**other)
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Inner::Owned { ptr, len } = self.inner {
            unsafe {
                let layout = Layout::from_size_align_unchecked(len, BUFFER_ALIGN);
                crate::allocator::EVM.dealloc(ptr.as_ptr(), layout);
            }
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
            Inner::Empty => Self::empty(),
            Inner::Owned { .. } => Self::from_slice(self),
            Inner::Account { key, data, range } => Self::new(Inner::Account {
                key: *key,
                data: *data,
                range: range.clone(),
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
            Inner::Empty => serializer.serialize_unit_variant("evm_buffer", 0, "empty"),
            Inner::Owned { ptr, len } => {
                let slice = unsafe { std::slice::from_raw_parts(ptr.as_ptr(), *len) };
                let bytes = serde_bytes::Bytes::new(slice);
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
                    0 => variant.unit_variant().map(|_| Buffer::empty()),
                    1 => variant.newtype_variant().map(Buffer::from_slice),
                    2 => variant.struct_variant(&["key", "range"], self),
                    _ => Err(serde::de::Error::unknown_variant(
                        "_",
                        &["empty", "owned", "account"],
                    )),
                }
            }
        }

        deserializer.deserialize_enum("evm_buffer", &["empty", "owned", "account"], BufferVisitor)
    }
}
