//! Solana Backend for rust evm
use evm::{
    backend::{Basic},
    H160, H256, U256
};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    instruction::{Instruction},
    entrypoint::ProgramResult,
};
use crate::{
    solidity_account::SolidityAccount,
    utils::keccak256_h256
};

/// Account storage
/// Trait to access account info
#[allow(clippy::redundant_closure_for_method_calls)]
pub trait AccountStorage {
    /// Apply function to given account
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U;

    /// Get contract address
    fn contract(&self) -> H160;
    /// Get caller address
    fn origin(&self) -> H160;
    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;

    /// Get solana address for given ethereum account
    fn get_account_solana_address(&self, address: &H160) -> Option<Pubkey> { self.apply_to_account(address, || None, |account| Some(account.get_solana_address())) }
    /// Check if ethereum account exists
    fn exists(&self, address: &H160) -> bool { self.apply_to_account(address, || false, |_| true) }
    /// Get account basic info (balance and nonce)
    fn basic(&self, address: &H160) -> Basic { self.apply_to_account(address, || Basic{balance: U256::zero(), nonce: U256::zero()}, |account| account.basic()) }
    /// Get code hash
    fn code_hash(&self, address: &H160) -> H256 { self.apply_to_account(address, || keccak256_h256(&[]) , |account| account.code_hash()) }
    /// Get code size
    fn code_size(&self, address: &H160) -> usize { self.apply_to_account(address, || 0, |account| account.code_size()) }
    /// Get code data
    fn code(&self, address: &H160) -> Vec<u8> { self.apply_to_account(address, Vec::new, |account| account.get_code()) }
    /// Get valids data
    fn valids(&self, address: &H160) -> Vec<u8> { self.apply_to_account(address, Vec::new, |account| account.get_valids()) }
    /// Get data from storage
    fn storage(&self, address: &H160, index: &U256) -> U256 { self.apply_to_account(address, U256::zero, |account| account.get_storage(index)) }
    /// Get account seeds
    fn seeds(&self, address: &H160) -> Option<(H160, u8)> {self.apply_to_account(address, || None, |account| Some(account.get_seeds())) }
    /// External call
    /// # Errors
    /// Will return `Err` if the external call returns err
    fn external_call(&self, _: &Instruction, _: &[AccountInfo]) -> ProgramResult { Ok(()) }
}

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(111)
}