use borsh::{BorshSerialize, BorshDeserialize};
use evm::{H160, U256, H256};
use solana_program::{pubkey::Pubkey};

use super::cache::AccountMeta;


#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum Action {
    ExternalInstruction {
        program_id: Pubkey,
        instruction: Vec<u8>,
        accounts: Vec<AccountMeta>,
        seeds: Vec<Vec<u8>>
    },
    NeonTransfer {
        source: H160,
        target: H160,
        value: U256,
    },
    NeonWithdraw {
        source: H160,
        value: U256,
    },
    EvmLog {
        address: H160,
        topics: Vec<H256>,
        data: Vec<u8>,
    },
    EvmSetStorage {
        address: H160,
        key: U256,
        value: U256,
    },
    EvmIncrementNonce {
        address: H160,
    },
    EvmSetCode {
        address: H160,
        code: Vec<u8>,
        valids: Vec<u8>,
    },
    EvmSelfDestruct {
        address: H160,
    },
}
