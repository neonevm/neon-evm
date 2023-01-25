use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::types::Address;

#[derive(Serialize, Deserialize)]
pub enum Action {
    ExternalInstruction {
        program_id: Pubkey,
        accounts: Vec<AccountMeta>,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        seeds: Vec<Vec<u8>>,
        allocate: usize,
    },
    NeonTransfer {
        source: Address,
        target: Address,
        #[serde(with = "ethnum::serde::bytes::le")]
        value: U256,
    },
    NeonWithdraw {
        source: Address,
        #[serde(with = "ethnum::serde::bytes::le")]
        value: U256,
    },
    EvmLog {
        address: Address,
        topics: Vec<[u8; 32]>,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
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
        D: serde::Deserializer<'de>
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor
        {
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
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}