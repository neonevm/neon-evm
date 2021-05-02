use crate::{
    account_data::AccountData,
    solana_backend::{AccountStorage, SolanaBackend},
    solidity_account::SolidityAccount,
    utils::keccak256_digest,
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
    accounts: Vec<SolidityAccount<'a>>,
    aliases: RefCell<Vec<(H160, usize)>>,
    clock_account: &'a AccountInfo<'a>,
    account_metas: Vec<&'a AccountInfo<'a>>,
    contract_id: H160,
    caller_id: H160,
}

impl<'a> ProgramAccountStorage<'a> {
    /// ProgramAccountStorage constructor
    /// 
    /// account_infos expectations: 
    /// 
    /// 0. contract account info
    /// 1. contract code info
    /// 2. caller or caller account info(for ether account)
    /// 3. ... other accounts
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>], clock_account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        debug_print!("account_storage::new");

        let account_info_iter = &mut account_infos.iter();

        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());
        let mut account_metas = Vec::with_capacity(account_infos.len());

        let mut push_account = |sol_account: SolidityAccount<'a>, account_info: &'a AccountInfo<'a>| {
            aliases.push((sol_account.get_ether(), accounts.len()));
            accounts.push(sol_account);
            account_metas.push(account_info);
        };

        let construct_contract_account = |account_info: &'a AccountInfo<'a>, code_info: &'a AccountInfo<'a>,| -> Result<SolidityAccount<'a>, ProgramError>
        {
            let account_data = AccountData::unpack(&account_info.data.borrow())?;
            let account = account_data.get_account()?;
    
            if *code_info.key != account.code_account {
                return Err(ProgramError::InvalidAccountData)
            }
    
            let code_data = code_info.data.clone();
            let code_acc = AccountData::unpack(&code_data.borrow())?;
            code_acc.get_contract()?;
    
            Ok( SolidityAccount::new(account_info.key, (*account_info.lamports.borrow()).clone(), account_data, Some((code_acc, code_data)))? )
        };

        let contract_id = {
            let program_info = next_account_info(account_info_iter)?;
            let program_code = next_account_info(account_info_iter)?;

            let contract_acc = construct_contract_account(program_info, program_code)?;
            let contract_id = contract_acc.get_ether();
            push_account(contract_acc, program_info);

            contract_id
        };

        let caller_id = {
            let caller_info = next_account_info(account_info_iter)?;

            let caller_id: H160 = if caller_info.owner == program_id {
                let account_data = AccountData::unpack(&caller_info.data.borrow())?;
                account_data.get_account()?;

                let caller_acc = SolidityAccount::new(caller_info.key, (*caller_info.lamports.borrow()).clone(), account_data, None)?;

                let caller_id = caller_acc.get_ether();
                push_account(caller_acc, caller_info);

                caller_id
            } else {
                if !caller_info.is_signer {
                    debug_print!("Caller mast be signer");
                    debug_print!("Caller pubkey: {}", &caller_info.key.to_string());

                    return Err(ProgramError::InvalidArgument);
                }

                keccak256_digest(&caller_info.key.to_bytes()).into()
            };

            caller_id
        };

        while let Ok(account_info) = next_account_info(account_info_iter) {
            if account_info.owner == program_id {
                let account_data = AccountData::unpack(&account_info.data.borrow())?;
                let account = match account_data {
                    AccountData::Account(ref acc) => acc,
                    _ => { continue; },
                };

                let sol_account = if account.code_account == Pubkey::new_from_array([0u8; 32]) {
                    debug_print!("Common account");

                    SolidityAccount::new(account_info.key, (*account_info.lamports.borrow()).clone(), account_data, None)?
                } else {
                    debug_print!("Contract account");
                    let code_info = next_account_info(account_info_iter)?;

                    construct_contract_account(account_info, code_info)?
                };

                push_account(sol_account, account_info);
            }
        }

        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);

        Ok(Self {
            accounts: accounts,
            aliases: RefCell::new(aliases),
            clock_account,
            account_metas: account_metas,
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
            Some(&self.accounts[pos])
        } else {
            None
        }
    }

    pub fn apply<A, I>(&mut self, values: A, delete_empty: bool) -> Result<(), ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
    {
        let system_account = SolanaBackend::<ProgramAccountStorage>::system_account();
        let system_account_ecrecover = SolanaBackend::<ProgramAccountStorage>::system_account_ecrecover();

        for apply in values {
            match apply {
                Apply::Modify {address, basic, code, storage, reset_storage} => {
                    if (address == system_account) || (address == system_account_ecrecover) {
                        continue;
                    }
                    if let Some(pos) = self.find_account(&address) {
                        let account = &mut self.accounts[pos];
                        let account_info = &self.account_metas[pos];
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
