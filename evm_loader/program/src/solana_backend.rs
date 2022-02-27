//! Solana Backend for rust evm

use crate::{
    account_data::{AccountData, ACCOUNT_SEED_VERSION},
    solidity_account::SolidityAccount,
    token::{get_token_account_data, get_token_mint_data},
    utils::keccak256_h256,
};
use evm::{H160, H256, U256};
use solana_program::{
    account_info::AccountInfo,
    clock::Epoch,
    pubkey::Pubkey,
};
use std::{cell::RefCell, rc::Rc};

/// Account information for `apply_to_solana_account`.
pub struct AccountStorageInfo<'a> {
    /// The lamports in account
    pub lamports: u64,
    /// The data held in account
    pub data: Rc<RefCell<&'a mut [u8]>>,
    /// Program that owns account
    pub owner: &'a Pubkey,
    /// This account's data contains a loaded program
    pub executable: bool,
    /// The epoch at which account will next owe rent
    pub rent_epoch: Epoch,
}

impl<'a> AccountStorageInfo<'a> {
    /// Creates new instance of `AccountStorageInfo` from `AccountInfo`.
    #[must_use]
    pub fn from(info: &'a AccountInfo<'a>) -> Self {
        Self {
            lamports: info.lamports(),
            data: info.data.clone(),
            owner: info.owner,
            executable: info.executable,
            rent_epoch: info.rent_epoch,
        }
    }
}

/// Account storage
/// Trait to access account info
#[allow(clippy::redundant_closure_for_method_calls)]
pub trait AccountStorage {
    /// Apply function to given account
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
    where
        F: FnOnce(&SolidityAccount) -> U,
        D: FnOnce() -> U;

    /// Apply function to given Solana account
    fn apply_to_solana_account<U, D, F>(&self, address: &Pubkey, d: D, f: F) -> U
    where
        F: FnOnce(/*info: */ &AccountStorageInfo) -> U,
        D: FnOnce() -> U;

    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;
    /// Get contract address
    fn contract(&self) -> H160;
    /// Get caller address
    fn origin(&self) -> H160;
    /// Get account balance
    fn balance(&self, address: &H160) -> U256;
    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block number
	fn block_hash(&self, number: U256) -> H256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;

    /// Get solana address for given ethereum account
    fn get_account_solana_address(&self, address: &H160) -> Pubkey;

    /// Get SPL token balance
    fn get_spl_token_balance(&self, token_account: &Pubkey) -> u64 {
        self.apply_to_solana_account(
            token_account,
            || 0_u64,
            |info| get_token_account_data(&info.data.borrow(), info.owner).map_or(0, |a| a.amount)
        )
    }

    /// Get SPL token supply
    fn get_spl_token_supply(&self, token_mint: &Pubkey) -> u64 {
        self.apply_to_solana_account(
            token_mint,
            || 0_u64,
            |info| get_token_mint_data(&info.data.borrow(), info.owner).map_or(0, |mint| mint.supply)
        )
    }

    /// Get SPL token decimals
    fn get_spl_token_decimals(&self, token_mint: &Pubkey) -> u8 {
        self.apply_to_solana_account(
            token_mint,
            || 0_u8,
            |info| get_token_mint_data(&info.data.borrow(), info.owner).map_or(0_u8, |mint| mint.decimals)
        )
    }

    /// Get ERC20 token account address and bump seed
    fn get_erc20_token_address(&self, owner: &H160, contract: &H160, mint: &Pubkey) -> (Pubkey, u8) {
        let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ERC20Balance", &mint.to_bytes(), contract.as_bytes(), owner.as_bytes()];
        Pubkey::find_program_address(seeds, self.program_id())
    }

    /// Get ERC20 allowance account address and bump seed
    fn get_erc20_allowance_address(&self, owner: &H160, spender: &H160, contract: &H160, mint: &Pubkey) -> (Pubkey, u8) {
        let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ERC20Allowance", &mint.to_bytes(), contract.as_bytes(), owner.as_bytes(), spender.as_bytes()];
        Pubkey::find_program_address(seeds, self.program_id())
    }

    /// Get ERC20 allowance
    fn get_erc20_allowance(&self, owner: &H160, spender: &H160, contract: &H160, mint: &Pubkey) -> U256 {
        let (allowance_address, _) = self.get_erc20_allowance_address(owner, spender, contract, mint);

        let account_data = self.apply_to_solana_account(
            &allowance_address,
            || None,
            |info| AccountData::unpack(&info.data.borrow()).ok()
        );

        account_data
            .and_then(|d| d.get_erc20_allowance().ok().map(|a| a.value))
            .unwrap_or_else(U256::zero)
    }

    /// Check if ethereum account exists
    fn exists(&self, address: &H160) -> bool {
        self.apply_to_account(address, || false, |_| true)
    }

    /// Get account nonce
    fn nonce(&self, address: &H160) -> U256 {
        let nonce = self.apply_to_account(
            address,
            || 0_u64,
            |account| account.get_nonce(),
        );

        U256::from(nonce)
    }

    /// Get code hash
    fn code_hash(&self, address: &H160) -> H256 {
        self.apply_to_account(
            address,
            || keccak256_h256(&[]),
            |account| account.code_hash(),
        )
    }

    /// Get code size
    fn code_size(&self, address: &H160) -> usize {
        self.apply_to_account(address, || 0, |account| account.code_size())
    }

    /// Get code data
    fn code(&self, address: &H160) -> Vec<u8> {
        self.apply_to_account(address, Vec::new, |account| account.get_code())
    }

    /// Get valids data
    fn valids(&self, address: &H160) -> Vec<u8> {
        self.apply_to_account(address, Vec::new, |account| account.get_valids())
    }

    /// Get data from storage
    fn storage(&self, address: &H160, index: &U256) -> U256 {
        self.apply_to_account(address, U256::zero, |account| account.get_storage(index))
    }

    /// Get account seeds
    fn seeds(&self, address: &H160) -> Option<(H160, u8)> {
        self.apply_to_account(address, || None, |account| Some(account.get_seeds()))
    }
}
