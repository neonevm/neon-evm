use hex::FromHex;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::Deref;

/// TODO Maybe replace with #[serde(with = "hex")], but pay attention to "0x" prefix missing from "hex" serialization
/// Wrapper structure around vector of bytes.
#[derive(Debug, PartialEq, Eq, Default, Hash, Clone)]
pub struct HexBytes(pub Vec<u8>);

impl HexBytes {
    /// Simple constructor.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> HexBytes {
        HexBytes(bytes)
    }
}

impl Deref for HexBytes {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<u8>> for HexBytes {
    fn from(bytes: Vec<u8>) -> HexBytes {
        HexBytes(bytes)
    }
}

impl From<HexBytes> for Vec<u8> {
    fn from(value: HexBytes) -> Self {
        value.0
    }
}

impl Serialize for HexBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = "0x".to_owned();
        value.push_str(hex::encode(&self.0).as_str());
        serializer.serialize_str(value.as_ref())
    }
}

impl<'a> Deserialize<'a> for HexBytes {
    fn deserialize<D>(deserializer: D) -> Result<HexBytes, D::Error>
    where
        D: Deserializer<'a>,
    {
        deserializer.deserialize_any(BytesVisitor)
    }
}

struct BytesVisitor;

impl<'a> Visitor<'a> for BytesVisitor {
    type Value = HexBytes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a 0x-prefixed, hex-encoded vector of bytes")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() >= 2 && value.starts_with("0x") && value.len() & 1 == 0 {
            Ok(HexBytes::new(FromHex::from_hex(&value[2..]).map_err(
                |e| serde::de::Error::custom(format!("Invalid hex: {e}")),
            )?))
        } else {
            Err(serde::de::Error::custom(
                "Invalid bytes format. Expected a 0x-prefixed hex string with even length",
            ))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(value.as_ref())
    }
}
