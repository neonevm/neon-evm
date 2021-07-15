//! `AccountStorage` for solana program realisation
use crate::{
    account_data::AccountData,
    solana_backend::{AccountStorage, SolanaBackend},
    solidity_account::SolidityAccount,
    utils::keccak256_h256,
    token::{get_token_account_balance, token_mint},
};
use evm::backend::Apply;
use evm::{H160,  U256};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    pubkey::Pubkey,
    instruction::Instruction,
    program_error::ProgramError,
    sysvar::{clock, clock::Clock, Sysvar},
    program::invoke_signed,
    entrypoint::ProgramResult,
};
use std::{
    cell::RefCell,
};
use std::convert::TryFrom;

/// Sender
pub enum Sender {
    /// Ethereum account address
    Ethereum (H160),
    /// Solana account ethereum address
    Solana (H160),
}

struct AccountMeta<'a> {
    account: &'a AccountInfo<'a>,
    #[allow(dead_code)]
    token: &'a AccountInfo<'a>,
    code: Option<&'a AccountInfo<'a>>
}

/// `AccountStorage` for solana program realization
pub struct ProgramAccountStorage<'a> {
    accounts: Vec<SolidityAccount<'a>>,
    aliases: RefCell<Vec<(H160, usize)>>,
    clock_account: &'a AccountInfo<'a>,
    account_metas: Vec<AccountMeta<'a>>,
    contract_id: H160,
    sender: Sender,
}

fn check_token_account(token: &AccountInfo, account: &AccountInfo) -> Result<(), ProgramError> {
    debug_print!("check_token_account");
    if *token.owner != spl_token::id() {
        debug_print!("token.owner != spl_token::id() {}", token.owner);
        return Err(ProgramError::IncorrectProgramId);
    }

    let data = account.try_borrow_data()?;
    let data = AccountData::unpack(&data)?;
    let data = data.get_account()?;
    if data.eth_token_account != *token.key {
        debug_print!("data.eth_token_account != *token.key data.eth = {} token.key = {}", data.eth_token_account, *token.key);
        return Err(ProgramError::IncorrectProgramId);
    }

    debug_print!("check_token_account success");

    Ok(())
}

impl<'a> ProgramAccountStorage<'a> {
    /// `ProgramAccountStorage` constructor
    /// 
    /// `account_infos` expectations: 
    /// 
    /// 0. contract account info
    /// 1. contract code info
    /// 2. caller or caller account info(for ether account)
    /// 3. ... other accounts (with `clock_account` in any place)
    /// 
    /// # Errors
    ///
    /// Will return: 
    /// `ProgramError::InvalidArgument` if account in `account_infos` is wrong or in wrong place
    /// `ProgramError::InvalidAccountData` if account's data doesn't meet requirements
    /// `ProgramError::NotEnoughAccountKeys` if `account_infos` doesn't meet expectations
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>]) -> Result<Self, ProgramError> {
        debug_print!("account_storage::new");

        let account_info_iter = &mut account_infos.iter();

        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());
        let mut account_metas = Vec::with_capacity(account_infos.len());

        let mut clock_account = None;

        let mut push_account = |sol_account: SolidityAccount<'a>, account_info: &'a AccountInfo<'a>, token_info: &'a AccountInfo<'a>, code_info: Option<&'a AccountInfo<'a>>| {
            aliases.push((sol_account.get_ether(), accounts.len()));
            accounts.push(sol_account);
            account_metas.push(AccountMeta{account: account_info, token: token_info, code: code_info});
        };

        let construct_contract_account = |account_info: &'a AccountInfo<'a>, token_info: &'a AccountInfo<'a>, code_info: &'a AccountInfo<'a>,| -> Result<SolidityAccount<'a>, ProgramError>
        {
            if account_info.owner != program_id || code_info.owner != program_id {
                debug_print!("Invalid owner for program info/code");
                return Err(ProgramError::InvalidArgument);
            }

            let account_data = AccountData::unpack(&account_info.data.borrow())?;
            let account = account_data.get_account()?;
    
            if *code_info.key != account.code_account {
                debug_print!("code_info.key: {:?}", *code_info.key);
                debug_print!("account.code_account: {:?}", account.code_account);
                return Err(ProgramError::InvalidAccountData)
            }
    
            let code_data = code_info.data.clone();
            let code_acc = AccountData::unpack(&code_data.borrow())?;
            code_acc.get_contract()?;
    
            Ok(SolidityAccount::new(account_info.key, get_token_account_balance(token_info)?, account_data, Some((code_acc, code_data))))
        };

        let contract_id = {
            let program_info = next_account_info(account_info_iter)?;
            let program_token = next_account_info(account_info_iter)?;
            let program_code = next_account_info(account_info_iter)?;

            check_token_account(program_token, program_info)?;

            let contract_acc = construct_contract_account(program_info, program_token, program_code)?;
            let contract_id = contract_acc.get_ether();
            push_account(contract_acc, program_info, program_token, Some(program_code));

            contract_id
        };

        let sender = {
            let caller_info = next_account_info(account_info_iter)?;
            
            if caller_info.owner == program_id {
                let caller_token_info = next_account_info(account_info_iter)?;
                check_token_account(caller_token_info, caller_info)?;

                let account_data = AccountData::unpack(&caller_info.data.borrow())?;
                account_data.get_account()?;
                
                let caller_acc = SolidityAccount::new(caller_info.key, get_token_account_balance(caller_token_info)?, account_data, None);
                let caller_address = caller_acc.get_ether();
                push_account(caller_acc, caller_info, caller_token_info, None);
                Sender::Ethereum(caller_address)
            } else {
                if !caller_info.is_signer {
                    debug_print!("Caller must be signer");
                    debug_print!("Caller pubkey: {}", &caller_info.key.to_string());

                    return Err(ProgramError::InvalidArgument);
                }

                Sender::Solana(keccak256_h256(&caller_info.key.to_bytes()).into())
            }
        };

        while let Ok(account_info) = next_account_info(account_info_iter) {
            if account_info.owner == program_id {
                let account_data = AccountData::unpack(&account_info.data.borrow())?;
                let account = match account_data {
                    AccountData::Account(ref acc) => acc,
                    _ => { continue; },
                };

                let token_info = next_account_info(account_info_iter)?;
                check_token_account(token_info, account_info)?;

                let (sol_account, code_info) = if account.code_account == Pubkey::new_from_array([0_u8; 32]) {
                    debug_print!("User account");
                    (SolidityAccount::new(account_info.key, get_token_account_balance(token_info)?, account_data, None) , None)
                } else {
                    debug_print!("Contract account");
                    let code_info = next_account_info(account_info_iter)?;

                    (construct_contract_account(account_info, token_info, code_info)?, Some(code_info))
                };

                push_account(sol_account, account_info, token_info, code_info);
            } else if clock::check_id(account_info.key) {
                debug_print!("Clock account {}", account_info.key);
                clock_account = Some(account_info);
            }
        }

        let clock_account = if let Some(clock_acc) = clock_account {
            clock_acc
        } else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };


        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);

        Ok(Self {
            accounts,
            aliases: RefCell::new(aliases),
            clock_account,
            account_metas,
            contract_id,
            sender
        })
    }

    /// Get sender address
    pub const fn get_sender(&self) -> &Sender {
        &self.sender
    }

    /// Get contract `SolidityAccount`
    pub fn get_contract_account(&self) -> Option<&SolidityAccount<'a>> {
        self.get_account(&self.contract_id)
    }

    /// Get caller `SolidityAccount`
    pub fn get_caller_account(&self) -> Option<&SolidityAccount<'a>> {
        match self.sender {
            Sender::Ethereum(addr) => self.get_account(&addr),
            Sender::Solana(_addr) => None,
        }
    }

    fn find_account(&self, address: &H160) -> Option<usize> {
        let aliases = self.aliases.borrow();
        if let Ok(pos) = aliases.binary_search_by_key(&address, |v| &v.0) {
            debug_print!("Found account for {} on position {}", &address.to_string(), &pos.to_string());
            Some(aliases[pos].1)
        }
        else {
            debug_print!("Not found account for {}", &address.to_string());
            None
        }
    }

    fn get_account(&self, address: &H160) -> Option<&SolidityAccount<'a>> {
        self.find_account(address).map(|pos| &self.accounts[pos])
    }

    /// Apply contact execution results
    /// 
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply<A, I>(&mut self, values: A, _delete_empty: bool) -> Result<(), ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (U256, U256)>,
    {
        let system_account = SolanaBackend::<ProgramAccountStorage>::system_account();
        let system_account_ecrecover = SolanaBackend::<ProgramAccountStorage>::system_account_ecrecover();

        for apply in values {
            match apply {
                Apply::Modify {address, basic, code_and_valids, storage, reset_storage} => {
                    if (address == system_account) || (address == system_account_ecrecover) {
                        continue;
                    }
                    if let Some(pos) = self.find_account(&address) {
                        let account = &mut self.accounts[pos];
                        let AccountMeta{ account: account_info, token: _, code: _ } = &self.account_metas[pos];
                        let basic_balance = u64::try_from(basic.balance).map_err(|_| ProgramError::InvalidAccountData)?;
                        account.update(account_info, address, basic.nonce, basic_balance, &code_and_valids, storage, reset_storage)?;
                    } else {
                        if let Sender::Solana(addr) = self.sender {
                            if addr == address {
                                debug_print!("This is solana user, because {:?} == {:?}.", address, addr);
                                continue;
                            }
                        }
                        debug_print!("Apply can't be done. Not found account for address = {:?}.", address);
                        return Err(ProgramError::NotEnoughAccountKeys);
                    }
                },
                Apply::Delete { address } => {
                    debug_print!("Going to delete address = {:?}.", address);

                    if let Some(pos) = self.find_account(&address) {
                        let AccountMeta{ account: account_info, token: _, code: code_info } = &self.account_metas[pos];
                        let code_info = if let Some(code_info) = code_info {
                            code_info
                        } else {
                            debug_print!("Only contract account could be deleted. account = {:?} -> {:?}.", address, account_info.key);
                            return Err(ProgramError::InvalidAccountData);
                        };

                        let caller_account_index = self.find_account(&self.origin()).ok_or(ProgramError::NotEnoughAccountKeys)?;
                        let AccountMeta{ account: caller_info, token: _, code: _ } = &self.account_metas[caller_account_index];

                        debug_print!("Move funds from account");
                        **caller_info.lamports.borrow_mut() = caller_info.lamports() + account_info.lamports();
                        **account_info.lamports.borrow_mut() = 0;

                        debug_print!("Move funds from code");
                        **caller_info.lamports.borrow_mut() = caller_info.lamports() + code_info.lamports();
                        **code_info.lamports.borrow_mut() = 0;

                        debug_print!("Mark accounts empty");
                        let mut account_data = account_info.try_borrow_mut_data()?;
                        AccountData::pack(&AccountData::Empty, &mut account_data)?;
                        let mut code_data = code_info.try_borrow_mut_data()?;
                        AccountData::pack(&AccountData::Empty, &mut code_data)?;
                    } else {
                        debug_print!("Apply can't be done. Not found account for address = {:?}.", address);
                        return Err(ProgramError::NotEnoughAccountKeys);
                    }
                }
            }
        }

        //for log in logs {};

        Ok(())
    }

    /// Apply token transfers
    /// 
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_transfers(&mut self, _authority: &Pubkey, accounts: &[AccountInfo], transfers: Vec<evm::Transfer>) -> Result<(), ProgramError> {
        debug_print!("apply_transfers {:?}", transfers);

        for transfer in transfers {
            let source_account_index = self.find_account(&transfer.source).ok_or(ProgramError::NotEnoughAccountKeys)?;
            let AccountMeta{ account: source_account, token: source_token_account, code: _ } = &self.account_metas[source_account_index];
            let source_solidity_account = &self.accounts[source_account_index];
            
            let target_account_index = self.find_account(&transfer.target).ok_or(ProgramError::NotEnoughAccountKeys)?;
            let AccountMeta{ account: _, token: target_token_account, code: _ } = &self.account_metas[target_account_index];
            
            let amount = u64::try_from(transfer.value).map_err(|_| ProgramError::InvalidInstructionData)?;

            debug_print!("Transfer ETH tokens from {} to {} amount {}", source_token_account.key, target_token_account.key, amount);

            let instruction = spl_token::instruction::transfer_checked(
                &spl_token::id(),
                source_token_account.key,
                &token_mint::id(),
                target_token_account.key,
                source_account.key,
                &[],
                amount,
                token_mint::decimals(),
            )?;

            let (ether, nonce) = source_solidity_account.get_seeds();
            invoke_signed(&instruction, accounts, &[&[ether.as_bytes(), &[nonce]]])?;
        }

        debug_print!("apply_transfers done");

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
    fn origin(&self) -> H160 {
        match self.sender {
            Sender::Ethereum(value) | Sender::Solana(value) => value,
        }
    }

    fn block_number(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.unix_timestamp.into()
    }

    fn external_call(
        &self,
        instruction: &Instruction,
        account_infos: &[AccountInfo]
    ) -> ProgramResult {
        let (contract_eth, contract_nonce) = self.seeds(&self.contract()).unwrap();   // do_call already check existence of Ethereum account with such index
        let contract_seeds = [contract_eth.as_bytes(), &[contract_nonce]];

        match self.seeds(&self.origin()) {
            Some((sender_eth, sender_nonce)) => {
                let sender_seeds = [sender_eth.as_bytes(), &[sender_nonce]];
                invoke_signed(
                    instruction,
                    account_infos,
                    &[&sender_seeds[..], &contract_seeds[..]]
                )
            }
            None => {
                invoke_signed(
                    instruction,
                    account_infos,
                    &[&contract_seeds[..]]
                )
            }
        }
    }
}
