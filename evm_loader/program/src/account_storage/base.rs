use std::cell::{RefCell};
use std::collections::{BTreeMap, BTreeSet};
use evm::{H160};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use solana_program::sysvar::Sysvar;
use crate::account::{EthereumAccount, Operator, program};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};


impl<'a> ProgramAccountStorage<'a> {
    pub fn new(
        program_id: &'a Pubkey,
        operator: &Operator<'a>,
        system_program: Option<&program::System<'a>>,
        accounts: &'a [AccountInfo<'a>]
    ) -> Result<Self, ProgramError> {
        debug_print!("ProgramAccountStorage::new");

        let mut solana_accounts = BTreeMap::new();
        for account in accounts {
            let duplicate = solana_accounts.insert(*account.key, account);
            if duplicate.is_some() {
                return Err!(ProgramError::InvalidArgument; "Account {} - repeats in the transaction", account.key)
            }
        }

        solana_accounts.insert(*operator.key, operator.info);
        if let Some(system) = system_program {
            solana_accounts.insert(*system.key, system.into());
        }


        let mut ethereum_accounts = BTreeMap::new();
        for account_info in accounts {
            if account_info.owner != program_id {
                continue;
            }

            match crate::account::tag(program_id, account_info) {
                Ok(EthereumAccount::TAG) => {}
                Ok(_) | Err(_) => continue
            }

            let ether_account = EthereumAccount::from_account(program_id, account_info)?;
            ethereum_accounts.insert(ether_account.address, ether_account);
        }


        Ok(Self{
            program_id,
            operator: operator.key,
            clock: Clock::get()?,
            solana_accounts,
            ethereum_accounts,
            empty_ethereum_accounts: RefCell::new(BTreeSet::new()),
        })
    }

    pub fn add_ether_account(&mut self, program_id: &Pubkey, info: &'a AccountInfo<'a>) -> ProgramResult {
        let ether_account = EthereumAccount::from_account(program_id, info)?;
        let previous = self.ethereum_accounts.insert(ether_account.address, ether_account);
        assert!(previous.is_none());

        Ok(())
    }

    pub fn remove_ether_account(&mut self, address: &H160) -> Option<EthereumAccount<'a>> {
        self.ethereum_accounts.remove(address)
    }

    fn panic_if_account_not_exists(&self, address: &H160) {
        if self.ethereum_accounts.contains_key(address) {
            return;
        }

        let mut empty_accounts = self.empty_ethereum_accounts.borrow_mut();
        if empty_accounts.contains(address) {
            return;
        }

        let (solana_address, _) = self.calc_solana_address(address);
        if let Some(account) = self.solana_accounts.get(&solana_address) {
            assert!(system_program::check_id(account.owner), "Empty ethereum account {} must belong to the system program", address);

            empty_accounts.insert(*address);
            return;
        }

        panic!("Ethereum account {} must be present in the transaction", address);
    }

    pub fn solana_account(&self, solana_address: &Pubkey) -> Option<&'a AccountInfo<'a>> {
        self.solana_accounts.get(solana_address).copied()
    }

    pub fn ethereum_account(&self, address: &H160) -> Option<&EthereumAccount<'a>> {
        self.panic_if_account_not_exists(address);
        self.ethereum_accounts.get(address)
    }

    pub fn ethereum_account_mut(&mut self, address: &H160) -> &mut EthereumAccount<'a> {
        self.ethereum_accounts.get_mut(address).unwrap() // mutable accounts always present
    }

    pub fn block_accounts(&mut self, block: bool) -> Result<(), ProgramError> {
        for account in &mut self.ethereum_accounts.values_mut() {
            account.rw_blocked = block;
        }

        Ok(())
    }

    pub fn check_for_blocked_accounts(&self) -> Result<(), ProgramError> {
        for ethereum_account in self.ethereum_accounts.values() {
            ethereum_account.check_blocked()?;
        }

        Ok(())
    }
}
