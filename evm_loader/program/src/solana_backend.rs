//! Solana Backend for rust evm
use crate::{
    solidity_account::SolidityAccount, 
    utils::keccak256_h256,
};
use evm::{backend::Basic, H160, H256, U256};
use solana_program::{
    entrypoint::ProgramResult, instruction::Instruction, pubkey::Pubkey,
};
use crate::token::{get_token_account_data, get_token_mint_data};
use crate::account_data::{AccountData, ACCOUNT_SEED_VERSION};

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(111)
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
        F: FnOnce(/*data: */ &[u8], /*owner: */ &Pubkey) -> U,
        D: FnOnce() -> U;

    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;
    /// Get contract address
    fn contract(&self) -> H160;
    /// Get caller address
    fn origin(&self) -> H160;
    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;

    /// Get SPL token balance
    fn get_spl_token_balance(&self, token_account: &Pubkey) -> u64 {
        self.apply_to_solana_account(
            token_account,
            || 0_u64,
            |data, owner| get_token_account_data(data, owner).map_or(0, |a| a.amount)
        )
    }

    /// Get SPL token supply
    fn get_spl_token_supply(&self, token_mint: &Pubkey) -> u64 {
        self.apply_to_solana_account(
            token_mint,
            || 0_u64,
            |data, owner| get_token_mint_data(data, owner).map_or(0, |mint| mint.supply)
        )
    }

    /// Get SPL token decimals
    fn get_spl_token_decimals(&self, token_mint: &Pubkey) -> u8 {
        self.apply_to_solana_account(
            token_mint,
            || 0_u8,
            |data, owner| get_token_mint_data(data, owner).map_or(0_u8, |mint| mint.decimals)
        )
    }


    /// Get ERC20 allowance
    fn get_erc20_allowance(&self, owner: &H160, spender: &H160, mint: &Pubkey) -> U256 {
        let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ERC20Allowance", &mint.to_bytes(), owner.as_bytes(), spender.as_bytes()];
        let allowance_address = Pubkey::find_program_address(seeds, self.program_id()).0;

        let account_data = self.apply_to_solana_account(
            &allowance_address,
            || None,
            |data, _| AccountData::unpack(data).ok()
        );

        account_data
            .and_then(|d| d.get_erc20_allowance().ok().map(|a| a.value))
            .unwrap_or_else(U256::zero)
    }

    /// Get solana address for given ethereum account
    fn get_account_solana_address(&self, address: &H160) -> Option<Pubkey> {
        self.apply_to_account(
            address,
            || None,
            |account| Some(account.get_solana_address()),
        )
    }
    /// Check if ethereum account exists
    fn exists(&self, address: &H160) -> bool {
        self.apply_to_account(address, || false, |_| true)
    }
    /// Get account basic info (balance and nonce)
    fn basic(&self, address: &H160) -> Basic {
        self.apply_to_account(
            address,
            || Basic {
                balance: U256::zero(),
                nonce: U256::zero(),
            },
            |account| account.basic(),
        )
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
    /// External call
    /// # Errors
    /// Will return `Err` if the external call returns err
    fn external_call(&self, _: &Instruction) -> ProgramResult {
        Ok(())
    }
}
