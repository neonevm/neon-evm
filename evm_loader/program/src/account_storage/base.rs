use std::cell::{RefCell};
use std::collections::{BTreeMap, BTreeSet};
use evm::{H160, U256};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use solana_program::sysvar::Sysvar;
use crate::account::{EthereumAccount, Operator, program, TAG_EMPTY, EthereumStorage};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};


impl<'a> ProgramAccountStorage<'a> {
    pub fn new(
        program_id: &'a Pubkey,
        operator: &Operator<'a>,
        system_program: Option<&program::System<'a>>,
        accounts: &'a [AccountInfo<'a>]
    ) -> Result<Self, ProgramError> {
        debug_print!("ProgramAccountStorage::new");

        let mut solana_accounts = accounts.iter()
            .map(|a| (a.key, a))
            .collect::<BTreeMap<_, _>>();

        solana_accounts.insert(operator.key, operator.info);
        if let Some(system) = system_program {
            solana_accounts.insert(system.key, system.into());
        }


        let mut ethereum_accounts = BTreeMap::new();
        let mut storage_accounts = BTreeMap::new();

        for &account_info in solana_accounts.values() {
            if account_info.owner != program_id {
                continue;
            }

            match crate::account::tag(program_id, account_info) {
                Ok(EthereumAccount::TAG) => {
                    let account = EthereumAccount::from_account(program_id, account_info)?;
                    ethereum_accounts.insert(account.address, account);
                },
                Ok(EthereumStorage::TAG) => {
                    let account = EthereumStorage::from_account(program_id, account_info)?;
                    storage_accounts.insert((account.address, account.index), account);
                }
                Ok(_) | Err(_) => continue
            }
        }

        for storage in storage_accounts.values_mut() {
            let owner = &ethereum_accounts[&storage.address];
            if storage.generation != owner.generation {
                storage.clear(owner.generation, operator)?;
            }
        }

        Ok(Self {
            program_id,
            operator: operator.key,
            clock: Clock::get()?,
            solana_accounts,
            ethereum_accounts,
            empty_ethereum_accounts: RefCell::new(BTreeSet::new()),
            storage_accounts,
            empty_storage_accounts: RefCell::new(BTreeSet::new()),
        })
    }

    pub fn solana_account(&self, solana_address: &Pubkey) -> Option<&'a AccountInfo<'a>> {
        self.solana_accounts.get(solana_address).copied()
    }

    pub fn ethereum_storage(&self, address: H160, index: U256) -> Option<&EthereumStorage<'a>> {
        let key = (address, index);

        if let Some(account) = self.storage_accounts.get(&key) {
            return Some(account);
        }

        let mut empty_accounts = self.empty_storage_accounts.borrow_mut();
        if empty_accounts.contains(&key) {
            return None;
        }

        let solana_address = EthereumStorage::solana_address(self, &address, &index);
        if let Some(&account) = self.solana_accounts.get(&solana_address) {
            assert!(solana_program::system_program::check_id(account.owner));

            empty_accounts.insert(key);
            return None;
        }

        panic!(
            "Storage account {} {} (solana address {}) must be present in the transaction",
            address, index, solana_address
        );
    }

    pub fn ethereum_account(&self, address: &H160) -> Option<&EthereumAccount<'a>> {
        if let Some(account) = self.ethereum_accounts.get(address) {
            return Some(account);
        }

        let mut empty_accounts = self.empty_ethereum_accounts.borrow_mut();
        if empty_accounts.contains(address) {
            return None;
        }

        let (solana_address, _bump_seed) = self.calc_solana_address(address);
        if let Some(&account) = self.solana_accounts.get(&solana_address) {
            assert!(
                self.is_account_empty(account),
                "Empty ethereum account {} must belong to the system program or be uninitialized",
                address
            );

            empty_accounts.insert(*address);
            return None;
        }

        panic!(
            "Ethereum account {} (solana address {}) must be present in the transaction",
            address, solana_address
        );
    }

    pub fn ethereum_account_mut(&mut self, address: &H160) -> &mut EthereumAccount<'a> {
        self.ethereum_accounts.get_mut(address).unwrap() // mutable accounts always present
    }

    pub fn block_accounts(&mut self, block: bool) {
        for account in &mut self.ethereum_accounts.values_mut() {
            account.rw_blocked = block;
        }
    }

    pub fn check_for_blocked_accounts(&self) -> Result<(), ProgramError> {
        for ethereum_account in self.ethereum_accounts.values() {
            ethereum_account.check_blocked()?;
        }

        Ok(())
    }

    pub fn is_account_empty(&self, account: &AccountInfo) -> bool {
        system_program::check_id(account.owner) ||
            (account.owner == self.program_id() &&
                (account.data_is_empty() || account.data.borrow()[0] == TAG_EMPTY))
    }
}
