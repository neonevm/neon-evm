use std::cell::{RefCell};
use std::collections::{BTreeMap, BTreeSet};
use evm::{H160};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use solana_program::sysvar::Sysvar;
use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumContract, Operator, program};
use crate::account::ether_account::ContractExtension;
use crate::account_storage::{Account, ProgramAccountStorage};


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

    fn panic_if_account_not_exists(&self, address: &H160) {
        if self.ethereum_accounts.contains_key(address) {
            return;
        }

        let mut empty_accounts = self.empty_ethereum_accounts.borrow_mut();
        if empty_accounts.contains(address) {
            return;
        }

        let (solana_address, _) = Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], address.as_bytes()], self.program_id);
        if let Some(account) = self.solana_accounts.get(&solana_address) {
            assert!(system_program::check_id(account.owner), "Empty ethereum account {} must belong to the system program", address);

            empty_accounts.insert(*address);
            return;
        }

        panic!("Ethereum account {} must be present in the transaction", address);
    }

    pub fn ethereum_account(&self, address: &H160) -> Option<&EthereumAccount<'a>> {
        self.panic_if_account_not_exists(address);
        self.ethereum_accounts.get(address)
    }

    pub fn ethereum_account_mut(&mut self, address: &H160) -> &mut EthereumAccount<'a> {
        self.ethereum_accounts.get_mut(address).unwrap() // mutable accounts always present
    }

    pub fn ethereum_contract(&self, address: &H160) -> Option<&ContractExtension<'a>> {
        self.panic_if_account_not_exists(address);
        self.ethereum_accounts.get(address)?.extension.as_ref()
    }

    pub fn ethereum_contract_mut(&mut self, address: &H160) -> &mut ContractExtension<'a> {
        self.ethereum_accounts.get_mut(address).unwrap()
            .extension.as_mut().expect("Contract account is not created")
    }

    pub fn block_accounts(&mut self, block: bool) -> Result<(), ProgramError> {
        for account in &mut self.ethereum_accounts.values_mut() {
            if account.info.is_writable {
                account.rw_blocked = block;
            } else {
                account.ro_blocked_count = if block {
                    account.ro_blocked_count.checked_add(1)
                } else {
                    account.ro_blocked_count.checked_sub(1)
                }.ok_or_else(|| E!(ProgramError::InvalidAccountData; "Account {} - read lock overflow", account.address))?;
            }
        }

        Ok(())
    }

    pub fn check_for_blocked_accounts(&self, required_exclusive_access : bool) -> Result<(), ProgramError> {
        for ethereum_account in self.ethereum_accounts.values() {
            ethereum_account.check_blocked(required_exclusive_access)?;
        }

        Ok(())
    }
}
