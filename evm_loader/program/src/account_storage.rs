use crate::{
    account_data::AccountData,
    solana_backend::{AccountStorage, SolanaBackend},
    solidity_account::SolidityAccount,
};
use evm::backend::Apply;
use primitive_types::{H160, H256, U256};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
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
    contract_id: H160,
    caller_id: H160,
}

impl<'a> ProgramAccountStorage<'a> {
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>], clock_account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        debug_print!("account_storage::new");
        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());

        let mut contract_id: H160 = H160::zero();
        let mut caller_id: H160 = H160::zero();

        let account_info_iter = &mut account_infos.iter();
        let mut index: usize = 0;

        while let Ok(account_info) = next_account_info(account_info_iter) {
            if account_info.owner == program_id {
                let account_data = AccountData::unpack(&account_info.data.borrow())?;
                let account = AccountData::get_account(&account_data)?;

                let code_data = if account.code_account == Pubkey::new_from_array([0u8; 32]) {
                    debug_print!("Common account");

                    if caller_id == H160::zero() {
                        caller_id = account.ether;
                        debug_print!("caller id: {}", &caller_id.to_string());
                    }

                    None
                } else {
                    debug_print!("Contract account");

                    if contract_id == H160::zero() {
                        contract_id = account.ether;
                        debug_print!("contract id: {}", &contract_id.to_string());
                    }

                    let code_info = next_account_info(account_info_iter)?;
                    if *code_info.key != account.code_account {
                        return Err(ProgramError::InvalidAccountData)
                    }

                    let code_data = code_info.data.clone();
                    let code_acc = AccountData::unpack(&code_data.borrow())?;
                    AccountData::get_contract(&code_acc)?;

                    Some((code_acc, code_data))
                };

                let sol_account = SolidityAccount::new(account_info.key, (*account_info.lamports.borrow()).clone(), account_data, code_data)?;

                aliases.push((sol_account.get_ether(), index));
                accounts.push(Some(sol_account));
                index += 1;
            }
        }

        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);

        Ok(Self {
            accounts: accounts,
            aliases: RefCell::new(aliases),
            clock_account,
            account_infos: account_infos,
            contract_id: contract_id,
            caller_id: caller_id,
        })
    }

    pub fn get_contract_account(&self) -> Option<&SolidityAccount<'a>> {
        self.get_account(&self.contract_id)
    }

    pub fn get_caller_account(&self) -> Option<&SolidityAccount<'a>> {
        self.get_account(&self.caller_id)
    }

    fn find_account(&self, address: &H160) -> Option<usize> {
        let aliases = self.aliases.borrow();
        match aliases.binary_search_by_key(&address, |v| &v.0) {
            Ok(pos) => {
                debug_print!("Found account for {} on position {}", &address.to_string(), &pos.to_string());
                Some(aliases[pos].1)
            }
            Err(_) => {
                debug_print!("Not found account for {}", &address.to_string());
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

    fn contract(&self) -> H160 { self.contract_id }
    fn origin(&self) -> H160 { self.caller_id }

    fn block_number(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.unix_timestamp.into()
    }
}
