use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use crate::account::{EthereumAccount, ACCOUNT_SEED_VERSION};
use crate::executor::{Action, OwnedAccountInfo, OwnedAccountInfoPartial};
use evm::{H160, H256, U256};
use solana_program::{ pubkey::Pubkey };
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;

mod base;
mod apply;
mod backend;

#[derive(Debug)]
pub enum AccountOperation {
    Create {
        space: usize,
    },

    Resize {
        from: usize,
        to: usize,
    },
}

pub type AccountsOperations = Vec<(H160, AccountOperation)>;

#[derive(Debug, PartialEq, Eq)]
pub enum AccountsReadiness {
    Ready,
    NeedMoreReallocations,
}

pub struct ProgramAccountStorage<'a> {
    program_id: &'a Pubkey,
    operator: &'a Pubkey,
    clock: Clock,

    solana_accounts: BTreeMap<Pubkey, &'a AccountInfo<'a>>,
    ethereum_accounts: BTreeMap<H160, EthereumAccount<'a>>,
    empty_ethereum_accounts: RefCell<BTreeSet<H160>>,
}

/// Account storage
/// Trait to access account info
pub trait AccountStorage {
    /// Get `NEON` token mint
    fn neon_token_mint(&self) -> &Pubkey;

    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;

    /// Get operator pubkey
    fn operator(&self) -> &Pubkey;

    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;
    /// Get block hash
    fn block_hash(&self, number: U256) -> H256;
    /// Get chain id
    fn chain_id(&self) -> u64;

    /// Check if ethereum account exists
    fn exists(&self, address: &H160) -> bool;
    /// Get account nonce
    fn nonce(&self, address: &H160) -> U256;
    /// Get account balance
    fn balance(&self, address: &H160) -> U256;

    /// Get code size
    fn code_size(&self, address: &H160) -> usize;
    /// Get code hash
    fn code_hash(&self, address: &H160) -> H256;
    /// Get code data
    fn code(&self, address: &H160) -> Vec<u8>;
    /// Get valids data
    fn valids(&self, address: &H160) -> Vec<u8>;
    /// Get contract generation
    fn generation(&self, address: &H160) -> u32;

    /// Get storage account address and bump seed
    fn get_storage_address(&self, address: &H160, index: &U256) -> (Pubkey, u8) {
        let generation_bytes = self.generation(address).to_le_bytes();

        let mut index_bytes = [0_u8; 32];
        index.to_little_endian(&mut index_bytes);

        let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ContractStorage", address.as_bytes(), &generation_bytes, &index_bytes];
        Pubkey::find_program_address(seeds, self.program_id())
    }

    /// Get data from storage
    fn storage(&self, address: &H160, index: &U256) -> U256;

    /// Clone existing solana account
    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo;

    /// Clone part of existing solana account
    fn clone_solana_account_partial(&self, address: &Pubkey, offset: usize, len: usize) -> Option<OwnedAccountInfoPartial>;

    /// Calculate account solana address and bump seed
    fn calc_solana_address(&self, address: &H160) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], address.as_bytes()], self.program_id())
    }

    /// Resolve account solana address and bump seed
    fn solana_address(&self, address: &H160) -> (Pubkey, u8) {
        self.calc_solana_address(address)
    }

    /// Solana account data len
    fn solana_account_space(&self, address: &H160) -> Option<usize>;

    fn calc_accounts_operations(
        &self,
        actions: &Option<Vec<Action>>,
    ) -> AccountsOperations {
        let actions = match actions {
            None => return vec![],
            Some(actions) => actions,
        };

        let mut accounts = HashMap::new();
        for action in actions {
            let (address, code_size) = match action {
                Action::NeonTransfer { target, .. } => (target, 0),
                Action::EvmSetCode { address, code, .. } => (address, code.len()),
                _ => continue,
            };

            let space_needed = EthereumAccount::space_needed(code_size);
            if let Some(max_size) = accounts.get_mut(&address) {
                *max_size = space_needed.max(*max_size);
                continue;
            }
            accounts.insert(address, space_needed);
        }

        accounts.into_iter()
            .filter_map(|(address, space_needed)|
                match self.solana_account_space(address) {
                    None => Some((*address, AccountOperation::Create { space: space_needed })),
                    Some(space_current) if space_current < space_needed =>
                        Some((*address, AccountOperation::Resize { from: space_current, to: space_needed })),
                    _ => None,
                }
            ).collect()
    }
}
