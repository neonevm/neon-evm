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
        value: [u8; 32],
    },
    EvmIncrementNonce {
        address: Address,
    },
    EvmSetCode {
        address: Address,
        #[serde(with = "serde_bytes")]
        code: Vec<u8>,
    },
    EvmSelfDestruct {
        address: Address,
    },
}
