use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::types::{serde::bytes_32, Address};

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
        #[serde(with = "bytes_32")]
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
