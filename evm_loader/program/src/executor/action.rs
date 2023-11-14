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
    Transfer {
        source: Address,
        target: Address,
        chain_id: u64,
        #[serde(with = "ethnum::serde::bytes::le")]
        value: U256,
    },
    Burn {
        source: Address,
        chain_id: u64,
        #[serde(with = "ethnum::serde::bytes::le")]
        value: U256,
    },
    EvmSetStorage {
        address: Address,
        #[serde(with = "ethnum::serde::bytes::le")]
        index: U256,
        #[serde(with = "serde_bytes_32")]
        value: [u8; 32],
    },
    EvmIncrementNonce {
        address: Address,
        chain_id: u64,
    },
    EvmSetCode {
        address: Address,
        chain_id: u64,
        #[serde(with = "serde_bytes")]
        code: Vec<u8>,
    },
    EvmSelfDestruct {
        address: Address,
    },
}

pub fn filter_selfdestruct(actions: Vec<Action>) -> Vec<Action> {
    // Find all the account addresses which are scheduled to EvmSelfDestruct
    let accounts_to_destroy: std::collections::HashSet<_> = actions
        .iter()
        .filter_map(|action| match action {
            Action::EvmSelfDestruct { address } => Some(*address),
            _ => None,
        })
        .collect();

    actions
        .into_iter()
        .filter(|action| {
            match action {
                // We always apply ExternalInstruction for Solana accounts
                // and NeonTransfer + NeonWithdraw
                Action::ExternalInstruction { .. }
                | Action::Transfer { .. }
                | Action::Burn { .. } => true,
                // We remove EvmSetStorage|EvmIncrementNonce|EvmSetCode if account is scheduled for destroy
                Action::EvmSetStorage { address, .. }
                | Action::EvmSetCode { address, .. }
                | Action::EvmIncrementNonce { address, .. } => {
                    !accounts_to_destroy.contains(address)
                }
                // SelfDestruct is only aplied to contracts deployed in the current transaction
                Action::EvmSelfDestruct { .. } => false,
            }
        })
        .collect()
}

mod serde_bytes_32 {
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

    #[cfg(not(target_os = "solana"))]
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
