use std::cell::{RefCell};
use std::collections::{BTreeMap, BTreeSet};
use evm::{H160};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use solana_program::sysvar::Sysvar;
use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumContract, program};
use crate::account_storage::{Account, ProgramAccountStorage};



impl<'a> ProgramAccountStorage<'a> {
    pub fn new(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], chain_id: u64) -> Result<Self, ProgramError> {
        debug_print!("ProgramAccountStorage::new");

        let mut solana_accounts = BTreeMap::new();
        for account in accounts {
            let duplicate = solana_accounts.insert(*account.key, account);
            if duplicate.is_some() {
                return Err!(ProgramError::InvalidArgument; "Account {} - repeats in the transaction", account.key)
            }
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
            let ether_address = ether_account.address;

            let account = if let Some(code_account_key) = ether_account.code_account {
                debug_print!("Contract Account {}", ether_address);

                let code_account = solana_accounts[&code_account_key];
                let ether_contract = EthereumContract::from_account(program_id, code_account)?;
                Account::Contract(ether_account, ether_contract)
            } else {
                debug_print!("User Account {}", ether_address);
                Account::User(ether_account)
            };
            ethereum_accounts.insert(ether_address, account);
        }

        let token_program = solana_accounts.get(&spl_token::ID)
            .map(|info| program::Token::from_account(info))
            .transpose()?;


        Ok(Self{
            program_id,
            clock: Clock::get()?,
            token_program,
            solana_accounts,
            ethereum_accounts,
            empty_ethereum_accounts: RefCell::new(BTreeSet::new()),
            chain_id,
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

        #[allow(clippy::match_same_arms)]
        match self.ethereum_accounts.get(address)? {
            Account::User(ref account) => Some(account),
            Account::Contract(ref account, _) => Some(account),
        }
    }

    pub fn ethereum_account_mut(&mut self, address: &H160) -> Option<&mut EthereumAccount<'a>> {
        self.panic_if_account_not_exists(address);

        #[allow(clippy::match_same_arms)]
        match self.ethereum_accounts.get_mut(address)? {
            Account::User(ref mut account) => Some(account),
            Account::Contract(ref mut account, _) => Some(account),
        }
    }

    pub fn ethereum_contract(&self, address: &H160) -> Option<&EthereumContract<'a>> {
        self.panic_if_account_not_exists(address);

        match self.ethereum_accounts.get(address)? {
            Account::User(_) => None,
            Account::Contract(_, ref contract) => Some(contract),
        }
    }

    pub fn ethereum_contract_mut(&mut self, address: &H160) -> Option<&mut EthereumContract<'a>> {
        self.panic_if_account_not_exists(address);

        match self.ethereum_accounts.get_mut(address)? {
            Account::User(_) => None,
            Account::Contract(_, ref mut contract) => Some(contract),
        }
    }

    pub fn block_accounts(&mut self, block: bool) -> Result<(), ProgramError> {
        for ethereum_account in &mut self.ethereum_accounts.values_mut() {

            match ethereum_account {
                Account::User(account) => {
                    account.rw_blocked = block;
                }
                Account::Contract(account, contract) if contract.info.is_writable => {
                    account.rw_blocked = block;
                }
                Account::Contract(account, _contract) /* not is_writable */ => {
                    account.ro_blocked_count = if block {
                        account.ro_blocked_count.checked_add(1)
                    } else {
                        account.ro_blocked_count.checked_sub(1)
                    }.ok_or_else(|| E!(ProgramError::InvalidAccountData; "Account {} - read lock overflow", account.address))?;
                }
            }
        }

        Ok(())
    }

    pub fn check_for_blocked_accounts(&self, required_exclusive_access : bool) -> Result<(), ProgramError> {
        for ethereum_account in self.ethereum_accounts.values() {
            #[allow(clippy::match_same_arms)]
            match ethereum_account {
                Account::User(account) => account,
                Account::Contract(account, _) => account,
            }.check_blocked(required_exclusive_access)?;
        }

        Ok(())
    }
}
