use std::fmt::{Display, Formatter};

use borsh::{BorshDeserialize, BorshSerialize};
use evm::{H160, H256, U256};
use solana_program::pubkey::Pubkey;

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

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Action::ExternalInstruction { program_id, instruction, accounts, seeds } =>
                format!(
                    "ExternalInstruction (\
                        program_id = {}, \
                        instruction.len() = {}, \
                        accounts.len() = {}, \
                        seeds.len() = {}",
                    program_id,
                    instruction.len(),
                    accounts.len(),
                    seeds.len(),
                ),
            Action::NeonTransfer { source, target, value } =>
                format!("NeonTransfer (value = {}) from {} to {}", value, source, target),
            Action::NeonWithdraw { source, value } =>
                format!("NeonWithdraw from {} (value = {})", source, value),
            Action::EvmLog { address, topics, data } =>
                format!(
                    "EvmLog for {} (topics.len() = {}, data.len() = {})",
                    address,
                    topics.len(),
                    data.len(),
                ),
            Action::EvmSetStorage { address, key, value } =>
                format!("EvmSetStorage for {}, key = {}, value = {}", address, key, value),
            Action::EvmIncrementNonce { address } =>
                format!("EvmIncrementNonce for {}", address),
            Action::EvmSetCode { address, code, valids } =>
                format!(
                    "EvmSetCode for {} (code.len() = {}, valids.len() = {})",
                    address,
                    code.len(),
                    valids.len(),
                ),
            Action::EvmSelfDestruct { address } =>
                format!("EvmSelfDestruct for {}", address),
        };

        f.write_str(&msg)?;

        Ok(())
    }
}
