use crate::error::Result;
use crate::executor::OwnedAccountInfo;
use crate::types::Address;
use ethnum::U256;
use maybe_async::maybe_async;
use solana_program::account_info::AccountInfo;
#[cfg(target_os = "solana")]
use {crate::account::AccountsDB, solana_program::clock::Clock};

use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;

#[cfg(target_os = "solana")]
mod apply;
#[cfg(target_os = "solana")]
mod backend;
#[cfg(target_os = "solana")]
mod base;
mod block_hash;
pub use block_hash::find_slot_hash;

mod keys_cache;
pub use keys_cache::KeysCache;

#[cfg(target_os = "solana")]
pub struct ProgramAccountStorage<'a> {
    clock: Clock,
    rent: Rent,
    accounts: AccountsDB<'a>,
    keys: keys_cache::KeysCache,
}

/// Account storage
/// Trait to access account info
#[maybe_async(?Send)]
pub trait AccountStorage {
    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;
    /// Get operator pubkey
    fn operator(&self) -> Pubkey;

    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;
    /// Get block hash
    async fn block_hash(&self, number: u64) -> [u8; 32];

    /// Get rent info
    fn rent(&self) -> &Rent;

    /// Get return data from Solana
    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)>;

    /// Get account nonce
    async fn nonce(&self, address: Address, chain_id: u64) -> u64;
    /// Get account balance
    async fn balance(&self, address: Address, chain_id: u64) -> U256;

    fn is_valid_chain_id(&self, chain_id: u64) -> bool;
    fn chain_id_to_token(&self, chain_id: u64) -> Pubkey;
    fn default_chain_id(&self) -> u64;

    /// Get contract chain_id
    async fn contract_chain_id(&self, address: Address) -> Result<u64>;
    /// Get contract solana address
    fn contract_pubkey(&self, address: Address) -> (Pubkey, u8);

    /// Get code size
    async fn code_size(&self, address: Address) -> usize;
    /// Get code data
    async fn code(&self, address: Address) -> crate::evm::Buffer;

    /// Get data from storage
    async fn storage(&self, address: Address, index: U256) -> [u8; 32];

    /// Clone existing solana account
    async fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo;

    /// Map existing solana account
    async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R;
}
