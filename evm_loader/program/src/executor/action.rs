use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::types::Address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    ExternalInstruction {
        program_id: Pubkey,
        accounts: Vec<AccountMeta>,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        seeds: Vec<Vec<u8>>,
        fee: u64,
    },
    NeonTransfer {
        source: Address,
        target: Address,
        #[serde(with = "serde_u256")]
        value: U256,
    },
    NeonWithdraw {
        source: Address,
        #[serde(with = "serde_u256")]
        value: U256,
    },
    EvmSetStorage {
        address: Address,
        #[serde(with = "serde_u256")]
        index: U256,
        #[serde(with = "serde_bytes_32")]
        value: [u8; 32],
    },
    EvmIncrementNonce {
        address: Address,
    },
    EvmSetCode {
        address: Address,
        code: crate::evm::Buffer,
    },
    EvmSelfDestruct {
        address: Address,
    },
}

mod serde_bytes_32 {
    pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_bytes(value)
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

        deserializer.deserialize_bytes(BytesVisitor)
    }
}

mod serde_u256 {
    use ethnum::U256;
    use serde::{
        de::{self, SeqAccess, Visitor},
        Deserializer, Serializer,
    };

    pub fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&value.to_le_bytes())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(U256Visitor {})
    }

    struct U256Visitor {}

    impl<'de> Visitor<'de> for U256Visitor {
        type Value = U256;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("32 bytes in little endian")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let bytes = v
                .try_into()
                .map_err(|_| E::invalid_length(v.len(), &self))?;
            Ok(U256::from_le_bytes(bytes))
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let mut bytes = Vec::with_capacity(32);
            while let Some(b) = seq.next_element()? {
                bytes.push(b);
            }
            Ok(U256::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| de::Error::custom("Invalid U256 value"))?,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bincode() {
        let action = Action::EvmSetStorage {
            address: Address::default(),
            index: U256::from_le_bytes([
                255, 46, 185, 41, 144, 201, 3, 36, 227, 18, 148, 147, 106, 131, 110, 6, 229, 235,
                44, 154, 71, 124, 159, 144, 47, 119, 77, 5, 154, 49, 23, 54,
            ]),
            value: Default::default(),
        };
        let serialized = bincode::serialize(&action).unwrap();
        let _deserialized: Action = bincode::deserialize(&serialized).unwrap();
    }

    #[cfg(feature = "library")]
    #[test]
    fn roundtrip_json() {
        let action = Action::EvmSetStorage {
            address: Address::default(),
            index: U256::from_le_bytes([
                255, 46, 185, 41, 144, 201, 3, 36, 227, 18, 148, 147, 106, 131, 110, 6, 229, 235,
                44, 154, 71, 124, 159, 144, 47, 119, 77, 5, 154, 49, 23, 54,
            ]),
            value: Default::default(),
        };
        let serialized = serde_json::to_string(&action).unwrap();
        let _deserialized: Action = serde_json::from_str(&serialized).unwrap();
    }
}
