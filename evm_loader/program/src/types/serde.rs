// use ethnum::U256;

// serde_with::serde_conv!(
//     pub U256AsWords,
//     U256,
//     |value: &U256| { value.into_words() },
//     |words: (u128, u128)| -> Result<_, std::convert::Infallible> { Ok(U256::from_words(words.0, words.1)) }
// );

pub mod option_u256 {
    use std::fmt::{self, Formatter};

    use ethnum::U256;
    use serde::{de::Visitor, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<U256>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = value {
            serializer.serialize_bytes(&value.to_le_bytes())
        } else {
            serializer.serialize_bytes(&[])
        }
    }

    struct BytesVisitor;

    impl<'de> Visitor<'de> for BytesVisitor {
        type Value = Option<U256>;

        fn expecting(&self, f: &mut Formatter) -> fmt::Result {
            f.write_str(concat!("32 bytes in little endian"))
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v.is_empty() {
                return Ok(None);
            }

            let bytes = v
                .try_into()
                .map_err(|_| E::invalid_length(v.len(), &self))?;

            Ok(Some(U256::from_le_bytes(bytes)))
        }
    }

    #[doc(hidden)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(BytesVisitor)
    }
}

pub mod bytes_32 {
    pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&hex::encode(value))
        } else {
            serializer.serialize_bytes(value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor {
            type Value = [u8; 32];

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("[u8; 32]")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                use serde::de::Unexpected::Str;

                let value = hex::decode(value)
                    .map_err(|_| serde::de::Error::invalid_value(Str(value), &self))?;

                let value_len = value.len();
                value
                    .try_into()
                    .map_err(|_| serde::de::Error::invalid_length(value_len, &self))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value
                    .try_into()
                    .map_err(|_| serde::de::Error::invalid_length(value.len(), &self))
            }

            fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let mut bytes = Vec::with_capacity(32);
                while let Some(b) = seq.next_element()? {
                    bytes.push(b);
                }
                bytes
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("Invalid [u8; 32] value"))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(BytesVisitor)
        } else {
            deserializer.deserialize_bytes(BytesVisitor)
        }
    }
}
