use crate::account::{
    AccountsDB, BalanceAccount, ContractAccount, Operator, StorageCell, Treasury,
};
use crate::account_storage::ProgramAccountStorage;
use crate::config::DEFAULT_CHAIN_ID;
use crate::error::Result;
use crate::types::Address;
use ethnum::U256;
use solana_program::clock::Clock;
use solana_program::system_program;
use solana_program::sysvar::Sysvar;

use super::keys_cache::KeysCache;

impl<'a> ProgramAccountStorage<'a> {
    pub fn new(accounts: AccountsDB<'a>) -> Result<Self> {
        Ok(Self {
            clock: Clock::get()?,
            accounts,
            keys: KeysCache::new(),
        })
    }

    pub fn operator(&self) -> &Operator<'a> {
        self.accounts.operator()
    }

    pub fn operator_balance(&mut self) -> &mut BalanceAccount<'a> {
        self.accounts.operator_balance()
    }

    pub fn treasury(&self) -> &Treasury<'a> {
        self.accounts.treasury()
    }

    pub fn db(&self) -> &AccountsDB<'a> {
        &self.accounts
    }

    pub fn storage_cell(&self, address: Address, index: U256) -> Result<StorageCell<'a>> {
        let pubkey = self.keys.storage_cell(&crate::ID, address, index);

        let account = self.accounts.get(&pubkey);
        let result = StorageCell::from_account(&crate::ID, account.clone());

        if result.is_err() {
            // Check that account is not in a legacy format
            // Correct account can ether be owned by System or be valid StorageCell
            assert!(system_program::check_id(account.owner));
        }

        result
    }

    pub fn contract_account(&self, address: Address) -> Result<ContractAccount<'a>> {
        let pubkey = self.keys.contract(&crate::ID, address);

        let account = self.accounts.get(&pubkey);
        let result = ContractAccount::from_account(&crate::ID, account.clone());

        if result.is_err() {
            let legacy_tag = crate::account::legacy::TAG_ACCOUNT_CONTRACT_DEPRECATED;
            assert!(crate::account::validate_tag(&crate::ID, account, legacy_tag).is_err());
        }

        result
    }

    pub fn balance_account(&self, address: Address, chain_id: u64) -> Result<BalanceAccount<'a>> {
        let pubkey = self.keys.balance(&crate::ID, address, chain_id);

        let account = self.accounts.get(&pubkey);
        let result = BalanceAccount::from_account(&crate::ID, account.clone(), Some(address));

        if result.is_err() && (chain_id == DEFAULT_CHAIN_ID) {
            let contract_pubkey = self.keys.contract(&crate::ID, address);
            let contract = self.accounts.get(&contract_pubkey);

            let legacy_tag = crate::account::legacy::TAG_ACCOUNT_CONTRACT_DEPRECATED;
            assert!(crate::account::validate_tag(&crate::ID, contract, legacy_tag).is_err());
        }

        result
    }

    pub fn create_balance_account(
        &self,
        address: Address,
        chain_id: u64,
    ) -> Result<BalanceAccount<'a>> {
        BalanceAccount::create(address, chain_id, &self.accounts, Some(&self.keys))
    }
}
