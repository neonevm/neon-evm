//! `AccountStorage` for solana program realisation
use crate::{
    account_data::{AccountData, ACCOUNT_SEED_VERSION},
    solana_backend::{AccountStorage, AccountStorageInfo},
    solidity_account::SolidityAccount,
    // utils::keccak256_h256,
    token::{transfer_neon_token, get_token_account_data},
    precompile_contracts::is_precompile_address,
    system::create_pda_account
};
use evm::backend::Apply;
use evm::{H160,  U256};
use solana_program::{
    account_info::{AccountInfo},
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::{clock::Clock, Sysvar},
    program::invoke_signed,
    entrypoint::ProgramResult,
    system_program,
};
use std::{
    collections::BTreeMap,
    cell::RefCell
};
use crate::executor_state::{SplTransfer, ERC20Approve, SplApprove};
use crate::account_data::ERC20Allowance;



/// `AccountStorage` for solana program realization
pub struct ProgramAccountStorage<'a> {
    solana_accounts: BTreeMap<Pubkey, &'a AccountInfo<'a>>,
    solidity_accounts: Vec<SolidityAccount<'a>>,
    empty_solidity_accounts: RefCell<BTreeMap<H160, &'a AccountInfo<'a>>>,
    contract: H160,
    caller: H160,
    program_id: Pubkey
}

impl<'a> ProgramAccountStorage<'a> {
    /// `ProgramAccountStorage` constructor
    ///
    /// # Errors
    ///
    /// Will return: 
    /// `ProgramError::InvalidArgument` if account in `account_infos` is wrong or in wrong place
    /// `ProgramError::InvalidAccountData` if account's data doesn't meet requirements
    /// `ProgramError::NotEnoughAccountKeys` if `account_infos` doesn't meet expectations
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>]) -> Result<Self, ProgramError> {
        debug_print!("account_storage::new");

        let mut solana_accounts = BTreeMap::new();
        for info in account_infos {
            solana_accounts.insert(*info.key, info);
        }

        let mut solidity_accounts = Vec::new();
        for account_info in account_infos {
            if account_info.owner != program_id {
                continue;
            }

            let account_data = AccountData::unpack(&account_info.data.borrow())?;
            let account = match account_data.get_account() {
                Ok(account) => account,
                Err(_) => continue
            };

            let code = if account.code_account == Pubkey::new_from_array([0_u8; 32]) {
                None
            } else {
                let code_info = solana_accounts[&account.code_account];
                let code_data = AccountData::unpack(&code_info.data.borrow())?;
                Some((code_data, code_info.data.clone()))
            };

            let solidity_account = SolidityAccount::new(account_info.key, account_data, code);
            solidity_accounts.push(solidity_account);
        }

        let contract = solidity_accounts[0].get_ether();
        let caller = solidity_accounts[1].get_ether();

        solidity_accounts.sort_by_key(SolidityAccount::get_ether);

        Ok(ProgramAccountStorage{
            solana_accounts,
            solidity_accounts,
            empty_solidity_accounts: RefCell::new(BTreeMap::new()),
            contract,
            caller,
            program_id: *program_id
        })
    }

    #[must_use]
    fn get_solidity_account_index(&self, address: &H160) -> usize {
        self.solidity_accounts.binary_search_by_key(address, SolidityAccount::get_ether)
            .unwrap_or_else(|_| panic!("Solidity account {} must be present in the transaction", address))
    }

    #[must_use]
    fn get_solidity_account(&self, address: &H160) -> Option<&SolidityAccount<'a>> {
        if let Ok(index) = self.solidity_accounts.binary_search_by_key(address, SolidityAccount::get_ether) {
            return Some(&self.solidity_accounts[index]);
        }

        let mut empty_accounts = self.empty_solidity_accounts.borrow_mut();
        if empty_accounts.contains_key(address) {
            return None;
        }

        let (solana_address, _) = Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], address.as_bytes()], self.program_id());
        if let Some(account) = self.solana_accounts.get(&solana_address) {
            assert!(system_program::check_id(account.owner), "Empty solidity account {} must belong to the system program", address);

            empty_accounts.insert(*address, account);
            return None;
        }

        panic!("Solidity account {} must be present in the transaction", address)
    }

    /// Get contract `SolidityAccount`
    #[must_use]
    pub fn get_contract_account(&self) -> &SolidityAccount<'a> {
        self.get_solidity_account(&self.contract)
            .expect("Contract account must always present in the transaction")
    }

    /// Get caller `SolidityAccount`
    #[must_use]
    pub fn get_caller_account(&self) -> &SolidityAccount<'a> {
        self.get_solidity_account(&self.caller)
            .expect("Caller account must always present in the transaction")
    }

    /// Get caller `AccountInfo`
    #[must_use]
    pub fn get_caller_account_info(&self) -> &AccountInfo<'a> {
        let caller = self.get_caller_account();
        self.solana_accounts[caller.get_solana_address()]
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
        operator: &AccountInfo<'a>,
        _delete_empty: bool
    ) -> Result<u64, ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (U256, U256)>,
    {
        let mut allocated_space: u64 = 0;
        for apply in values {
            match apply {
                Apply::Modify {address, nonce, code_and_valids, storage, reset_storage} => {
                    if is_precompile_address(&address) {
                        continue;
                    }
                    
                    let index = self.get_solidity_account_index(&address);
                    let account = &mut self.solidity_accounts[index];
                    let account_info = self.solana_accounts[account.get_solana_address()];
                    allocated_space +=  account.update(account_info, address, nonce, &code_and_valids, storage, reset_storage)?;
                },
                Apply::Delete { address } => {
                    debug_print!("Going to delete address = {:?}.", address);

                    let index = self.get_solidity_account_index(&address);
                    let account = &mut self.solidity_accounts[index];
                    let account_info = self.solana_accounts[account.get_solana_address()];
                    let code_info = self.solana_accounts[account.get_code_solana_address()];

                    debug_print!("Move funds from account");
                    **operator.lamports.borrow_mut() += account_info.lamports();
                    **account_info.lamports.borrow_mut() = 0;

                    debug_print!("Move funds from code");
                    **operator.lamports.borrow_mut() += code_info.lamports();
                    **code_info.lamports.borrow_mut() = 0;

                    debug_print!("Mark accounts empty");
                    let mut account_data = account_info.try_borrow_mut_data()?;
                    AccountData::pack(&AccountData::Empty, &mut account_data)?;
                    let mut code_data = code_info.try_borrow_mut_data()?;
                    AccountData::pack(&AccountData::Empty, &mut code_data)?;
                }
            }
        }

        Ok(allocated_space)
    }

    /// Apply value token transfers
    /// 
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::NotEnoughAccountKeys` if need to apply changes to missing account
    /// or `account.update` errors
    pub fn apply_transfers(&mut self, accounts: &'a [AccountInfo<'a>], transfers: Vec<evm::Transfer>) -> Result<(), ProgramError> {
        debug_print!("apply_transfers {:?}", transfers);

        for transfer in transfers {
            let source = self.get_solidity_account(&transfer.source)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Solidity account {} must be initialized", transfer.source))?;
            let source_account = self.solana_accounts[source.get_solana_address()];
            let source_token_account = self.solana_accounts[source.get_neon_token_solana_address()];

            let target = self.get_solidity_account(&transfer.target)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Solidity account {} must be initialized", transfer.target))?;
            let target_token_account = self.solana_accounts[target.get_neon_token_solana_address()];

            transfer_neon_token(
                accounts,
                source_token_account,
                target_token_account,
                source_account,
                source,
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
            let source = self.get_solidity_account(&transfer.source)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Solidity account {} must be initialized", transfer.source))?;

            let instruction = spl_token::instruction::transfer(
                &spl_token::id(),
                &transfer.source_token,
                &transfer.target_token,
                source.get_solana_address(),
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
            let source = self.get_solidity_account(&approve.owner).ok_or_else(|| E!(ProgramError::NotEnoughAccountKeys))?;
            let (source_token, _) = self.get_erc20_token_address(&approve.owner, &approve.contract, &approve.mint);

            let instruction = spl_token::instruction::approve(
                &spl_token::id(),
                &source_token,
                &approve.spender,
                source.get_solana_address(),
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
    pub fn apply_erc20_approves(&mut self, accounts: &[AccountInfo], operator: &AccountInfo, approves: Vec<ERC20Approve>) -> ProgramResult {
        debug_print!("apply_erc20_approves {:?}", approves);

        for approve in approves {
            let data = AccountData::ERC20Allowance(ERC20Allowance{
                owner: approve.owner,
                spender: approve.spender,
                contract: approve.contract,
                mint: approve.mint,
                value: approve.value
            });

            let (account_address, bump_seed) = self.get_erc20_allowance_address(&approve.owner, &approve.spender, &approve.contract, &approve.mint);

            let seeds: &[&[u8]] = &[
                &[ACCOUNT_SEED_VERSION],
                b"ERC20Allowance",
                &approve.mint.to_bytes(),
                approve.contract.as_bytes(),
                approve.owner.as_bytes(),
                approve.spender.as_bytes(),
                &[bump_seed]
            ];

            let account = self.solana_accounts[&account_address];
            if account.data_is_empty() {
                create_pda_account(
                    self.program_id(),
                    accounts,
                    account,
                    seeds,
                    operator.key,
                    data.size()
                )?;
            }

            data.pack(&mut account.try_borrow_mut_data()?)?;
        }

        debug_print!("apply_erc20_approves done");

        Ok(())
    }
}

impl<'a> AccountStorage for ProgramAccountStorage<'a> {
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
        where F: FnOnce(&SolidityAccount) -> U,
              D: FnOnce() -> U
    {
        self.get_solidity_account(address).map_or_else(d, f)
    }

    fn apply_to_solana_account<U, D, F>(&self, address: &Pubkey, _d: D, f: F) -> U
        where F: FnOnce(/*info: */ &AccountStorageInfo) -> U,
              D: FnOnce() -> U
    {
        self.solana_accounts.get(address).map_or_else(
            || panic!("Solana account {} must be present in the transaction", address),
            |account_info| f(&AccountStorageInfo::from(account_info)))
    }

    fn program_id(&self) -> &Pubkey { &self.program_id }
    
    fn contract(&self) -> H160 {
        self.contract
    }

    fn origin(&self) -> H160 {
        self.caller
    }

    fn balance(&self, address: &H160) -> U256 {
        self.get_solidity_account(address).map_or_else(U256::zero, |account| {
            let token_account = &self.solana_accounts[account.get_neon_token_solana_address()];
            let token = get_token_account_data(&token_account.data.borrow(), token_account.owner)
                .expect("Invalid token account");
    
            U256::from(token.amount) * crate::token::eth::min_transfer_value()
        })
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
        self.get_solidity_account(address).is_some()
    }

    fn get_account_solana_address(&self, address: &H160) -> Pubkey {
        self.get_solidity_account(address).map_or_else(
            || {
                let empty_accounts = self.empty_solidity_accounts.borrow();
                *empty_accounts[address].key
            },
            |account| {
                *account.get_solana_address()
            }
        )
    }
}
