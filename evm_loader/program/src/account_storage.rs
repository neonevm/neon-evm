//! `AccountStorage` for solana program realisation
use crate::{
    account_data::{AccountData, ACCOUNT_SEED_VERSION},
    solana_backend::{AccountStorage},
    solidity_account::SolidityAccount,
    // utils::keccak256_h256,
    token::{get_token_account_balance, check_token_account, transfer_token},
    precompile_contracts::is_precompile_address
};
use evm::backend::Apply;
use evm::{H160,  U256};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    pubkey::Pubkey,
    instruction::Instruction,
    program_error::ProgramError,
    sysvar::{clock::Clock, Sysvar},
    program::invoke_signed,
    entrypoint::ProgramResult,
};
use std::{
    cell::RefCell,
    collections::BTreeMap
};
use crate::executor_state::{SplTransfer, ERC20Approve, SplApprove};
use solana_program::system_instruction::create_account;
use solana_program::rent::Rent;
use crate::account_data::ERC20Allowance;
use spl_associated_token_account::get_associated_token_address;

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
    solana_accounts: BTreeMap<Pubkey, &'a AccountInfo<'a>>,
    accounts: Vec<SolidityAccount<'a>>,
    aliases: RefCell<Vec<(H160, usize)>>,
    account_metas: Vec<AccountMeta<'a>>,
    contract_id: H160,
    sender: Sender,
    program_id: Pubkey
}


impl<'a> ProgramAccountStorage<'a> {
    /// `ProgramAccountStorage` constructor
    /// 
    /// `account_infos` expectations: 
    /// 
    /// 0. contract account info
    /// 1. contract code info
    /// 2. caller or caller account info(for ether account)
    /// 3. ... other accounts
    /// 
    /// # Errors
    ///
    /// Will return: 
    /// `ProgramError::InvalidArgument` if account in `account_infos` is wrong or in wrong place
    /// `ProgramError::InvalidAccountData` if account's data doesn't meet requirements
    /// `ProgramError::NotEnoughAccountKeys` if `account_infos` doesn't meet expectations
    #[allow(clippy::too_many_lines)]
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>]) -> Result<Self, ProgramError> {
        debug_print!("account_storage::new");

        let mut solana_accounts = BTreeMap::new();
        for info in account_infos {
            solana_accounts.insert(*info.key, info);
        }

        let account_info_iter = &mut account_infos.iter();

        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());
        let mut account_metas = Vec::with_capacity(account_infos.len());

        let mut push_account = |sol_account: SolidityAccount<'a>, account_info: &'a AccountInfo<'a>, token_info: &'a AccountInfo<'a>, code_info: Option<&'a AccountInfo<'a>>| {
            aliases.push((sol_account.get_ether(), accounts.len()));
            accounts.push(sol_account);
            account_metas.push(AccountMeta{account: account_info, token: token_info, code: code_info});
        };

        let construct_contract_account = |account_info: &'a AccountInfo<'a>, token_info: &'a AccountInfo<'a>, code_info: &'a AccountInfo<'a>,| -> Result<SolidityAccount<'a>, ProgramError>
        {
            if account_info.owner != program_id || code_info.owner != program_id {
                return Err!(ProgramError::InvalidArgument; "Invalid owner! account_info.owner={:?}, code_info.owner={:?}, program_id={:?}", account_info.owner, code_info.owner, program_id);
            }

            let account_data = AccountData::unpack(&account_info.data.borrow())?;
            let account = account_data.get_account()?;
    
            if *code_info.key != account.code_account {
                return Err!(ProgramError::InvalidAccountData; "code_info.key={:?}, account.code_account={:?}", *code_info.key, account.code_account)
            }
    
            let code_data = code_info.data.clone();
            let code_acc = AccountData::unpack(&code_data.borrow())?;
            code_acc.get_contract()?;
    
            Ok(SolidityAccount::new(account_info.key, get_token_account_balance(token_info)?, account_data, Some((code_acc, code_data))))
        };

        let contract_id = {
            let program_info = next_account_info(account_info_iter)?;
            let program_token = next_account_info(account_info_iter)?;
            
            check_token_account(program_token, program_info)?;

            let account_data = AccountData::unpack(&program_info.data.borrow())?;
            let account = account_data.get_account()?;

            let (contract_acc, program_code) = if account.code_account == Pubkey::new_from_array([0_u8; 32]) {
                (SolidityAccount::new(program_info.key, get_token_account_balance(program_token)?, account_data, None) , None)
            } else {
                let program_code = next_account_info(account_info_iter)?;
                (construct_contract_account(program_info, program_token, program_code)?, Some(program_code))
            };
            
            let contract_id = contract_acc.get_ether();
            push_account(contract_acc, program_info, program_token, program_code);

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
                // TODO: EvmInstruction::Call
                // https://github.com/neonlabsorg/neon-evm/issues/188
                // Does not fit in current vision.
                // It is needed to update behavior for all system in whole.
                return Err!(ProgramError::InvalidArgument; "Caller could not be Solana user. It must be neon-evm owned account");

                // if !caller_info.is_signer {
                //     return Err!(ProgramError::InvalidArgument; "Caller must be signer. Caller pubkey: {} ", &caller_info.key.to_string());
                // }

                // Sender::Solana(keccak256_h256(&caller_info.key.to_bytes()).into())
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
            }
        }

        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);

        Ok(Self {
            solana_accounts,
            accounts,
            aliases: RefCell::new(aliases),
            account_metas,
            contract_id,
            sender,
            program_id: *program_id
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
            debug_print!("Found account for {:?} on position {}", &address, &pos.to_string());
            Some(aliases[pos].1)
        }
        else {
            debug_print!("Not found account for {:?}", &address);
            None
        }
    }

    fn get_account(&self, address: &H160) -> Option<&SolidityAccount<'a>> {
        self.find_account(address).map(|pos| &self.accounts[pos])
    }

    /// Get caller account info
    pub fn get_caller_account_info(&self) -> Option<&AccountInfo<'a>> {
        if let Some(account_index) = self.find_account(&self.origin()) {
            let AccountMeta{ account, token: _, code: _ } = &self.account_metas[account_index];
            return Some(account);
        }
        None
    }

    /// Apply contact execution results
    /// 
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply<A, I>(
        &mut self, values: A,
        operator: Option<&AccountInfo<'a>>,
        _delete_empty: bool
    ) -> Result<(), ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (U256, U256)>,
    {
        for apply in values {
            match apply {
                Apply::Modify {address, basic, code_and_valids, storage, reset_storage} => {
                    if is_precompile_address(&address) {
                        continue;
                    }
                    if let Some(pos) = self.find_account(&address) {
                        let account = &mut self.accounts[pos];
                        let AccountMeta{ account: account_info, token: _, code: _ } = &self.account_metas[pos];
                        account.update(account_info, address, basic.nonce, basic.balance, &code_and_valids, storage, reset_storage)?;
                    } else {
                        if let Sender::Solana(addr) = self.sender {
                            if addr == address {
                                debug_print!("This is solana user, because {:?} == {:?}.", address, addr);
                                continue;
                            }
                        }
                        return Err!(ProgramError::NotEnoughAccountKeys; "Apply can't be done. Not found account for address = {:?}.", address);
                    }
                },
                Apply::Delete { address } => {
                    debug_print!("Going to delete address = {:?}.", address);

                    if let Some(pos) = self.find_account(&address) {
                        let AccountMeta{ account: account_info, token: _, code: code_info } = &self.account_metas[pos];
                        let code_info = if let Some(code_info) = code_info {
                            code_info
                        } else {
                            return Err!(ProgramError::InvalidAccountData; "Only contract account could be deleted. account = {:?} -> {:?}.", address, account_info.key);
                        };

                        let recipient = if let Some(some_operator) = operator {
                            some_operator
                        } else {
                            self.get_caller_account_info().ok_or_else(|| E!(ProgramError::InvalidArgument))?
                        };

                        debug_print!("Move funds from account");
                        **recipient.lamports.borrow_mut() += account_info.lamports();
                        **account_info.lamports.borrow_mut() = 0;

                        debug_print!("Move funds from code");
                        **recipient.lamports.borrow_mut() += code_info.lamports();
                        **code_info.lamports.borrow_mut() = 0;

                        debug_print!("Mark accounts empty");
                        let mut account_data = account_info.try_borrow_mut_data()?;
                        AccountData::pack(&AccountData::Empty, &mut account_data)?;
                        let mut code_data = code_info.try_borrow_mut_data()?;
                        AccountData::pack(&AccountData::Empty, &mut code_data)?;
                    } else {
                        return Err!(ProgramError::NotEnoughAccountKeys; "Apply can't be done. Not found account for address = {:?}.", address);
                    }
                }
            }
        }

        //for log in logs {};

        Ok(())
    }

    /// Apply value token transfers
    /// 
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_transfers(&mut self, accounts: &[AccountInfo], transfers: Vec<evm::Transfer>) -> Result<(), ProgramError> {
        debug_print!("apply_transfers {:?}", transfers);

        for transfer in transfers {
            let source_account_index = self.find_account(&transfer.source).ok_or_else(||E!(ProgramError::NotEnoughAccountKeys))?;
            let AccountMeta{ account: source_account, token: source_token_account, code: _ } = &self.account_metas[source_account_index];
            let source_solidity_account = &self.accounts[source_account_index];

            let target_account_index = self.find_account(&transfer.target).ok_or_else(||E!(ProgramError::NotEnoughAccountKeys))?;
            let AccountMeta{ account: _, token: target_token_account, code: _ } = &self.account_metas[target_account_index];

            transfer_token(
                accounts,
                source_token_account,
                target_token_account,
                source_account,
                source_solidity_account,
                &transfer.value,
            )?;
        }

        debug_print!("apply_transfers done");

        Ok(())
    }

    /// Apply spl token transfers
    ///
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_spl_transfers(&mut self, accounts: &[AccountInfo], transfers: Vec<SplTransfer>) -> Result<(), ProgramError> {
        debug_print!("apply_spl_transfers {:?}", transfers);

        for transfer in transfers {
            let source = self.get_account(&transfer.source).ok_or_else(||E!(ProgramError::NotEnoughAccountKeys))?;

            let instruction = spl_token::instruction::transfer(
                &spl_token::id(),
                &transfer.source_token,
                &transfer.target_token,
                &source.get_solana_address(),
                &[],
                transfer.value
            )?;

            let (ether, nonce) = source.get_seeds();
            let program_seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], ether.as_bytes(), &[nonce]];
            invoke_signed(&instruction, accounts, &[program_seeds])?;
        }

        debug_print!("apply_spl_transfers done");

        Ok(())
    }

    /// Apply spl token approves
    ///
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_spl_approves(&mut self, accounts: &[AccountInfo], approves: Vec<SplApprove>) -> Result<(), ProgramError> {
        debug_print!("apply_spl_approves {:?}", approves);

        for approve in approves {
            let source = self.get_account(&approve.owner).ok_or_else(||E!(ProgramError::NotEnoughAccountKeys))?;
            let source_token = get_associated_token_address(&source.get_solana_address(), &approve.mint);

            let instruction = spl_token::instruction::approve(
                &spl_token::id(),
                &source_token,
                &approve.spender,
                &source.get_solana_address(),
                &[],
                approve.value
            )?;

            let (ether, nonce) = source.get_seeds();
            let program_seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], ether.as_bytes(), &[nonce]];
            invoke_signed(&instruction, accounts, &[program_seeds])?;
        }

        debug_print!("apply_spl_approves done");

        Ok(())
    }

    /// Apply ERC20 approves
    ///
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_erc20_approves(&mut self, accounts: &[AccountInfo], operator: Option<&AccountInfo>, approves: Vec<ERC20Approve>) -> ProgramResult {
        debug_print!("apply_erc20_approves {:?}", approves);

        for approve in approves {
            let data = AccountData::ERC20Allowance(ERC20Allowance{
                owner: approve.owner,
                spender: approve.spender,
                mint: approve.mint,
                value: approve.value
            });

            let seeds: &[&[u8]] = &[
                &[ACCOUNT_SEED_VERSION],
                b"ERC20Allowance",
                &approve.mint.to_bytes(),
                approve.owner.as_bytes(),
                approve.spender.as_bytes()
            ];
            let (account_address, bump_seed) = Pubkey::find_program_address(seeds, self.program_id());

            let bump_seed = &[bump_seed];
            let mut seeds = seeds.to_vec();
            seeds.push(bump_seed);

            let account = self.solana_accounts[&account_address];
            if account.data_is_empty() {
                let operator = match operator {
                    Some(operator) => operator.key,
                    None => return Err!(ProgramError::NotEnoughAccountKeys)
                };

                let rent = Rent::get()?;
                let balance = rent.minimum_balance(data.size());

                let instruction = create_account(
                    operator,
                    account.key,
                    balance,
                    data.size() as u64,
                    self.program_id()
                );
                invoke_signed(&instruction, accounts, &[&seeds])?;
            }

            data.pack(&mut account.try_borrow_mut_data()?)?;
        }

        debug_print!("apply_erc20_approves done");

        Ok(())
    }
}

impl<'a> AccountStorage for ProgramAccountStorage<'a> {
    fn apply_to_account<U, D, F>(&self, address: &H160, _d: D, f: F) -> U
        where F: FnOnce(&SolidityAccount) -> U,
              D: FnOnce() -> U
    {
        f(self.get_account(address).unwrap())
    }

    fn apply_to_solana_account<U, D, F>(&self, address: &Pubkey, _d: D, f: F) -> U
        where F: FnOnce(/*data: */ &[u8], /*owner: */ &Pubkey) -> U,
              D: FnOnce() -> U
    {
        let account_info = self.solana_accounts[address];
        f(&account_info.data.borrow(), account_info.owner)
    }

    fn program_id(&self) -> &Pubkey { &self.program_id }
    fn contract(&self) -> H160 { self.contract_id }
    fn origin(&self) -> H160 {
        match self.sender {
            Sender::Ethereum(value) | Sender::Solana(value) => value,
        }
    }

    fn block_number(&self) -> U256 {
        let clock = Clock::get().unwrap();
        clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        let clock = Clock::get().unwrap();
        clock.unix_timestamp.into()
    }

    fn exists(&self, address: &H160) -> bool {
        self.find_account(address).is_some()
    }

    fn external_call(
        &self,
        instruction: &Instruction
    ) -> ProgramResult {
        let (contract_eth, contract_nonce) = self.seeds(&self.contract()).unwrap();   // do_call already check existence of Ethereum account with such index
        let contract_seeds = [&[ACCOUNT_SEED_VERSION], contract_eth.as_bytes(), &[contract_nonce]];

        let mut account_infos: Vec<AccountInfo> = vec![
            self.solana_accounts[&instruction.program_id].clone()
        ];
        for meta in &instruction.accounts {
            account_infos.push(self.solana_accounts[&meta.pubkey].clone());
        }

        match self.seeds(&self.origin()) {
            Some((sender_eth, sender_nonce)) => {
                let sender_seeds = [&[ACCOUNT_SEED_VERSION], sender_eth.as_bytes(), &[sender_nonce]];
                invoke_signed(
                    instruction,
                    &account_infos,
                    &[&sender_seeds[..], &contract_seeds[..]]
                )
                // Todo: neon-evm does not return an external call error.
                // https://github.com/neonlabsorg/neon-evm/issues/120
                // debug_print!("invoke_signed done.");
                // debug_print!("invoke_signed returned: {:?}", program_result);
            }
            None => {
                invoke_signed(
                    instruction,
                    &account_infos,
                    &[&contract_seeds[..]]
                )
            }
        }
    }
}
