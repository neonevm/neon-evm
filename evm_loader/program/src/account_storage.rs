use crate::{
    solana_backend::{AccountStorage, SolanaBackend},
    solidity_account::SolidityAccount,
};
use evm::backend::Apply;
use primitive_types::{H160, H256, U256};
use solana_sdk::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::{clock::Clock, Sysvar},
};
use std::{
    cell::RefCell,
};

pub struct ProgramAccountStorage<'a> {
    accounts: Vec<Option<SolidityAccount<'a>>>,
    aliases: RefCell<Vec<(H160, usize)>>,
    clock_account: &'a AccountInfo<'a>,
    account_infos: &'a [AccountInfo<'a>],
}

impl<'a> ProgramAccountStorage<'a> {
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>], clock_account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        debug_print!("backend::new");
        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());

        for (i, account) in (&account_infos).iter().enumerate() {
            debug_print!(&i.to_string());
            if account.owner == program_id {
                let sol_account = SolidityAccount::new(account.key, account.data.clone(), (*account.lamports.borrow()).clone())?;
                aliases.push((sol_account.get_ether(), i));
                accounts.push(Some(sol_account));
            } else {
                accounts.push(None)
            }
        }
        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);
        Ok(Self {
            accounts: accounts,
            aliases: RefCell::new(aliases),
            clock_account,
            account_infos: account_infos,
        })
    }

    pub fn get_account_by_index(&self, index: usize) -> Option<&SolidityAccount<'a>> {
        if let Some(acc) = &self.accounts[index] {
            Some(&acc)
        } else {
            None
        }
    }

    fn find_account(&self, address: &H160) -> Option<usize> {
        let aliases = self.aliases.borrow();
        match aliases.binary_search_by_key(&address, |v| &v.0) {
            Ok(pos) => {
                debug_print!(&("Found account for ".to_owned() + &address.to_string() + " on position " + &pos.to_string()));
                Some(aliases[pos].1)
            }
            Err(_) => {
                debug_print!(&("Not found account for ".to_owned() + &address.to_string()));
                None
            }
        }
    }

    fn get_account(&self, address: &H160) -> Option<&SolidityAccount<'a>> {
        if let Some(pos) = self.find_account(address) {
            self.accounts[pos].as_ref()
        } else {
            None
        }
    }

    pub fn apply<A, I>(&mut self, values: A, delete_empty: bool, skip_addr: Option<(H160, bool)>) -> Result<(), ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
    {
        let ether_addr = skip_addr.unwrap_or_else(|| (H160::zero(), true));
        let system_account = SolanaBackend::<ProgramAccountStorage>::system_account();
        let system_account_ecrecover = SolanaBackend::<ProgramAccountStorage>::system_account_ecrecover();

        for apply in values {
            match apply {
                Apply::Modify {address, basic, code, storage, reset_storage} => {
                    if (address == system_account) || (address == system_account_ecrecover) {
                        continue;
                    }
                    if ether_addr.1 != true && address == ether_addr.0 {
                        continue;
                    }
                    if let Some(pos) = self.find_account(&address) {
                        let account = self.accounts[pos].as_mut().ok_or_else(|| ProgramError::NotEnoughAccountKeys)?;
                        let account_info = &self.account_infos[pos];
                        account.update(&account_info, address, basic.nonce, basic.balance.as_u64(), &code, storage, reset_storage)?;
                    }
                }
                Apply::Delete { address: _ } => {}
            }
        }

        //for log in logs {};

        Ok(())
    }
}

impl<'a> AccountStorage for ProgramAccountStorage<'a> {    
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U
    {
        self.get_account(address).map_or_else(d, f)
    }

    fn apply_to_contract<U, D, F>(&self, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U
    {
        self.get_account_by_index(0).map_or_else(d, f)
    }
        
    fn apply_to_caller<U, D, F>(&self, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U
    {
        self.get_account_by_index(1).map_or_else(d, f)
    }

    fn origin(&self) -> H160 { self.aliases.borrow()[1].0 }

    fn block_number(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.unix_timestamp.into()
    }
}