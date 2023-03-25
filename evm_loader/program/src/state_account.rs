use crate::{
    account::{program, EthereumAccount, FinalizedState, Holder, Incinerator, Operator, State},
    config::OPERATOR_PRIORITY_SLOTS,
    error::Error,
    types::{Address, Transaction},
};
use ethnum::U256;
use solana_program::{
    account_info::AccountInfo, clock::Clock, program_error::ProgramError, pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::cell::{Ref, RefMut};

const ACCOUNT_CHUNK_LEN: usize = 1 + 1 + 32;

pub enum Deposit<'a> {
    ReturnToOperator(Operator<'a>),
    Burn(Incinerator<'a>),
}

pub struct BlockedAccountMeta {
    pub key: Pubkey,
    pub exists: bool,
    pub is_writable: bool,
}

pub type BlockedAccounts = Vec<BlockedAccountMeta>;

impl<'a> FinalizedState<'a> {
    #[must_use]
    pub fn is_outdated(&self, transaction_hash: &[u8; 32]) -> bool {
        self.transaction_hash.ne(transaction_hash)
    }
}

impl<'a> State<'a> {
    pub fn new(
        program_id: &'a Pubkey,
        info: &'a AccountInfo<'a>,
        accounts: &crate::instruction::transaction_step::Accounts<'a>,
        caller: Address,
        trx: &Transaction,
    ) -> Result<Self, ProgramError> {
        let owner = match crate::account::tag(program_id, info)? {
            Holder::TAG => {
                let holder = Holder::from_account(program_id, info)?;
                holder.owner
            }
            FinalizedState::TAG => {
                let finalized_storage = FinalizedState::from_account(program_id, info)?;
                if !finalized_storage.is_outdated(&trx.hash) {
                    return Err!(Error::StorageAccountFinalized.into(); "Transaction already finalized");
                }

                finalized_storage.owner
            }
            _ => {
                return Err!(ProgramError::InvalidAccountData; "Account {} - expected finalized storage or holder", info.key)
            }
        };

        if &owner != accounts.operator.key {
            return Err!(ProgramError::InvalidAccountData; "Account {} - invalid state account owner", info.key);
        }

        let data = crate::account::state::Data {
            owner,
            transaction_hash: trx.hash,
            caller,
            gas_limit: trx.gas_limit,
            gas_price: trx.gas_price,
            gas_used: U256::ZERO,
            operator: *accounts.operator.key,
            slot: Clock::get()?.slot,
            accounts_len: accounts.remaining_accounts.len(),
        };

        info.data.borrow_mut()[0] = 0_u8;
        let mut storage = State::init(program_id, info, data)?;

        storage.make_deposit(&accounts.system_program, &accounts.operator)?;
        storage.write_blocked_accounts(program_id, accounts.remaining_accounts)?;
        Ok(storage)
    }

    pub fn restore(
        program_id: &Pubkey,
        info: &'a AccountInfo<'a>,
        operator: &Operator,
        remaining_accounts: &[AccountInfo],
        is_cancelling: bool,
    ) -> Result<(Self, BlockedAccounts), ProgramError> {
        let account_tag = crate::account::tag(program_id, info)?;
        if account_tag == FinalizedState::TAG {
            return Err!(Error::StorageAccountFinalized.into(); "Account {} - Storage Finalized", info.key);
        }
        if account_tag == Holder::TAG {
            return Err!(Error::StorageAccountUninitialized.into(); "Account {} - Storage Uninitialized", info.key);
        }

        let mut storage = State::from_account(program_id, info)?;
        let blocked_accounts =
            storage.check_blocked_accounts(program_id, remaining_accounts, is_cancelling)?;

        let clock = Clock::get()?;
        if (*operator.key != storage.operator)
            && ((clock.slot - storage.slot) <= OPERATOR_PRIORITY_SLOTS)
        {
            return Err!(ProgramError::InvalidAccountData; "operator.key != storage.operator");
        }

        if storage.operator != *operator.key {
            storage.operator = *operator.key;
            storage.slot = clock.slot;
        }

        Ok((storage, blocked_accounts))
    }

    pub fn finalize(self, deposit: Deposit<'a>) -> Result<FinalizedState<'a>, ProgramError> {
        debug_print!("Finalize Storage {}", self.info.key);

        match deposit {
            Deposit::ReturnToOperator(operator) => self.withdraw_deposit(&operator),
            Deposit::Burn(incinerator) => self.withdraw_deposit(&incinerator),
        }?;

        let finalized_data = crate::account::state::FinalizedData {
            owner: self.owner,
            transaction_hash: self.transaction_hash,
        };

        let finalized = unsafe { self.replace(finalized_data) }?;
        Ok(finalized)
    }

    fn make_deposit(
        &self,
        system_program: &program::System<'a>,
        source: &Operator<'a>,
    ) -> Result<(), ProgramError> {
        system_program.transfer(source, self.info, crate::config::PAYMENT_TO_DEPOSIT)
    }

    fn withdraw_deposit(&self, target: &AccountInfo<'a>) -> Result<(), ProgramError> {
        let source_lamports = self
            .info
            .lamports()
            .checked_sub(crate::config::PAYMENT_TO_DEPOSIT)
            .ok_or_else(
                || E!(ProgramError::InvalidArgument; "Deposit source lamports underflow"),
            )?;

        let target_lamports = target
            .lamports()
            .checked_add(crate::config::PAYMENT_TO_DEPOSIT)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Deposit target lamports overflow"))?;

        **self.info.lamports.borrow_mut() = source_lamports;
        **target.lamports.borrow_mut() = target_lamports;

        Ok(())
    }

    pub fn read_blocked_accounts(&self) -> Result<BlockedAccounts, ProgramError> {
        let (begin, end) = self.blocked_accounts_region();

        let account_data = self.info.try_borrow_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "Account {} - data too small, required: {}", self.info.key, end);
        }

        let keys_storage = &account_data[begin..end];
        let chunks = keys_storage.chunks_exact(ACCOUNT_CHUNK_LEN);
        let accounts = chunks
            .map(|c| c.split_at(2))
            .map(|(meta, key)| BlockedAccountMeta {
                key: Pubkey::try_from(key).expect("key is 32 bytes"),
                exists: meta[1] != 0,
                is_writable: meta[0] != 0,
            })
            .collect();

        Ok(accounts)
    }

    fn write_blocked_accounts(
        &mut self,
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> Result<(), ProgramError> {
        assert_eq!(self.accounts_len, accounts.len()); // should be always true

        let (begin, end) = self.blocked_accounts_region();

        let mut account_data = self.info.try_borrow_mut_data()?;
        if account_data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "Account {} - data too small, required: {}", self.info.key, end);
        }

        let accounts_storage = &mut account_data[begin..end];
        let accounts_storage = accounts_storage.chunks_exact_mut(ACCOUNT_CHUNK_LEN);
        for (info, account_storage) in accounts.iter().zip(accounts_storage) {
            account_storage[0] = u8::from(info.is_writable);
            account_storage[1] = u8::from(Self::account_exists(program_id, info));
            account_storage[2..].copy_from_slice(info.key.as_ref());
        }

        Ok(())
    }

    fn check_blocked_accounts(
        &self,
        program_id: &Pubkey,
        remaining_accounts: &[AccountInfo],
        is_cancelling: bool,
    ) -> Result<BlockedAccounts, ProgramError> {
        let blocked_accounts = self.read_blocked_accounts()?;
        if blocked_accounts.len() != remaining_accounts.len() {
            return Err!(ProgramError::NotEnoughAccountKeys; "Invalid number of accounts");
        }

        for (blocked, info) in blocked_accounts.iter().zip(remaining_accounts) {
            if blocked.key != *info.key {
                return Err!(ProgramError::InvalidAccountData; "Expected account {}, found {}", blocked.key, info.key);
            }

            if blocked.is_writable != info.is_writable {
                return Err!(ProgramError::InvalidAccountData; "Expected account {} is_writable: {}", info.key, blocked.is_writable);
            }

            if !is_cancelling && !blocked.exists && Self::account_exists(program_id, info) {
                return Err!(
                    ProgramError::AccountAlreadyInitialized;
                    "Blocked nonexistent account {} was created/initialized outside current transaction. \
                    Transaction is being cancelled in order to prevent possible data corruption.",
                    info.key
                );
            }
        }

        Ok(blocked_accounts)
    }

    #[must_use]
    pub fn evm_state_data(&self) -> Ref<[u8]> {
        let (_, accounts_region_end) = self.blocked_accounts_region();

        let data = self.info.data.borrow();
        Ref::map(data, |d| &d[accounts_region_end..])
    }

    #[must_use]
    pub fn evm_state_mut_data(&mut self) -> RefMut<[u8]> {
        let (_, accounts_region_end) = self.blocked_accounts_region();

        let data = self.info.data.borrow_mut();
        RefMut::map(data, |d| &mut d[accounts_region_end..])
    }

    #[must_use]
    fn blocked_accounts_region(&self) -> (usize, usize) {
        let begin = Self::SIZE;
        let end = begin + self.accounts_len * ACCOUNT_CHUNK_LEN;

        (begin, end)
    }

    #[must_use]
    fn account_exists(program_id: &Pubkey, info: &AccountInfo) -> bool {
        (info.owner == program_id)
            && !info.data_is_empty()
            && (info.data.borrow()[0] == EthereumAccount::TAG)
    }
}
