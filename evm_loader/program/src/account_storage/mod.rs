use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumContract, program};
use evm::{H160, H256, U256};
use solana_program::{ pubkey::Pubkey };
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;

mod base;
mod apply;
mod backend;


enum Account<'a> {
    User(EthereumAccount<'a>),
    Contract(EthereumAccount<'a>, EthereumContract<'a>),
}

pub struct ProgramAccountStorage<'a> {
    token_mint: Pubkey,
    program_id: &'a Pubkey,
    clock: Clock,
    token_program: Option<program::Token<'a>>,

    solana_accounts: BTreeMap<Pubkey, &'a AccountInfo<'a>>,
    ethereum_accounts: BTreeMap<H160, Account<'a>>,
    empty_ethereum_accounts: RefCell<BTreeSet<H160>>,

    chain_id: u64,
}

/// Account storage
/// Trait to access account info
pub trait AccountStorage {
    /// Get `NEON` token mint
    fn token_mint(&self) -> &Pubkey;

    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;

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

    /// Get SPL token balance
    fn get_spl_token_balance(&self, token_account: &Pubkey) -> u64;
    /// Get SPL token supply
    fn get_spl_token_supply(&self, token_mint: &Pubkey) -> u64;
    /// Get SPL token decimals
    fn get_spl_token_decimals(&self, token_mint: &Pubkey) -> u8;

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
    fn get_erc20_allowance(&self, owner: &H160, spender: &H160, contract: &H160, mint: &Pubkey) -> U256;

    /// Query account meta info
    fn query_account(&self, key: &Pubkey, data_offset: usize, data_len: usize) -> Option<crate::query::Value>;

    /// Solana accounts data len
    fn solana_accounts_space(&self, address: &H160) -> (usize, usize);
}
