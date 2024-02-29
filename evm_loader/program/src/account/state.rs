use std::cell::{Ref, RefMut};
use std::collections::BTreeMap;
use std::mem::size_of;

use crate::account_storage::AccountStorage;
use crate::config::DEFAULT_CHAIN_ID;
use crate::error::{Error, Result};
use crate::types::{Address, Transaction};
use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use super::{
    revision, AccountHeader, AccountsDB, BalanceAccount, Holder, StateFinalizedAccount,
    ACCOUNT_PREFIX_LEN, TAG_HOLDER, TAG_STATE, TAG_STATE_FINALIZED,
};

#[derive(PartialEq, Eq)]
pub enum AccountsStatus {
    Ok,
    RevisionChanged,
}

/// Storage data account to store execution metainfo between steps for iterative execution
#[derive(Serialize, Deserialize)]
struct Data {
    pub owner: Pubkey,
    pub transaction: Transaction,
    /// Ethereum transaction caller address
    pub origin: Address,
    /// Stored accounts
    pub revisions: BTreeMap<Pubkey, u32>,
    /// Ethereum transaction gas used and paid
    #[serde(with = "ethnum::serde::bytes::le")]
    pub gas_used: U256,
}

#[repr(C, packed)]
struct Header {
    pub evm_state_len: usize,
    pub evm_machine_len: usize,
    pub data_len: usize,
}
impl AccountHeader for Header {
    const VERSION: u8 = 0;
}

pub struct StateAccount<'a> {
    account: AccountInfo<'a>,
    data: Data,
}

const BUFFER_OFFSET: usize = ACCOUNT_PREFIX_LEN + size_of::<Header>();

impl<'a> StateAccount<'a> {
    #[must_use]
    pub fn into_account(self) -> AccountInfo<'a> {
        self.account
    }

    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        super::validate_tag(program_id, &account, TAG_STATE)?;

        let (offset, len) = {
            let header = super::header::<Header>(&account);
            let offset = BUFFER_OFFSET + header.evm_state_len + header.evm_machine_len;
            (offset, header.data_len)
        };

        let data = {
            let account_data = account.try_borrow_data()?;
            let buffer = &account_data[offset..(offset + len)];
            bincode::deserialize(buffer)?
        };

        Ok(Self { account, data })
    }

    pub fn new(
        program_id: &Pubkey,
        info: AccountInfo<'a>,
        accounts: &AccountsDB<'a>,
        origin: Address,
        transaction: Transaction,
    ) -> Result<Self> {
        let owner = match super::tag(program_id, &info)? {
            TAG_HOLDER => {
                let holder = Holder::from_account(program_id, info.clone())?;
                holder.validate_owner(accounts.operator())?;
                holder.owner()
            }
            TAG_STATE_FINALIZED => {
                let finalized = StateFinalizedAccount::from_account(program_id, info.clone())?;
                finalized.validate_owner(accounts.operator())?;
                finalized.validate_trx(&transaction)?;
                finalized.owner()
            }
            tag => return Err(Error::AccountInvalidTag(*info.key, tag)),
        };

        let revisions = accounts
            .into_iter()
            .map(|account| {
                let revision = revision(program_id, account).unwrap_or(0);
                (*account.key, revision)
            })
            .collect();

        let data = Data {
            owner,
            transaction,
            origin,
            revisions,
            gas_used: U256::ZERO,
        };

        super::set_tag(program_id, &info, TAG_STATE, Header::VERSION)?;
        {
            // Set header
            let mut header = super::header_mut::<Header>(&info);
            header.evm_state_len = 0;
            header.evm_machine_len = 0;
            header.data_len = 0;
        }

        Ok(Self {
            account: info,
            data,
        })
    }

    pub fn restore(
        program_id: &Pubkey,
        info: AccountInfo<'a>,
        accounts: &AccountsDB,
    ) -> Result<(Self, AccountsStatus)> {
        let mut state = Self::from_account(program_id, info)?;
        let mut status = AccountsStatus::Ok;

        for account in accounts {
            let account_revision = revision(program_id, account).unwrap_or(0);
            let stored_revision = state
                .data
                .revisions
                .entry(*account.key)
                .or_insert(account_revision);

            if stored_revision != &account_revision {
                status = AccountsStatus::RevisionChanged;
                *stored_revision = account_revision;
            }
        }

        Ok((state, status))
    }

    pub fn finalize(self, program_id: &Pubkey) -> Result<()> {
        debug_print!("Finalize Storage {}", self.account.key);

        // Change tag to finalized
        StateFinalizedAccount::convert_from_state(program_id, self)?;

        Ok(())
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Pubkey> {
        self.data.revisions.keys()
    }

    #[inline]
    #[must_use]
    fn header(&self) -> Ref<Header> {
        super::header(&self.account)
    }

    #[inline]
    #[must_use]
    fn header_mut(&mut self) -> RefMut<Header> {
        super::header_mut(&self.account)
    }

    #[must_use]
    pub fn buffer(&self) -> Ref<[u8]> {
        let data = self.account.data.borrow();
        Ref::map(data, |d| &d[BUFFER_OFFSET..])
    }

    #[must_use]
    pub fn buffer_mut(&mut self) -> RefMut<[u8]> {
        let data = self.account.data.borrow_mut();
        RefMut::map(data, |d| &mut d[BUFFER_OFFSET..])
    }

    #[must_use]
    pub fn buffer_variables(&self) -> (usize, usize) {
        let header = self.header();
        (header.evm_state_len, header.evm_machine_len)
    }

    pub fn set_buffer_variables(&mut self, evm_state_len: usize, evm_machine_len: usize) {
        let mut header = self.header_mut();
        header.evm_state_len = evm_state_len;
        header.evm_machine_len = evm_machine_len;
    }

    pub fn save_data(&mut self) -> Result<()> {
        let (evm_state_len, evm_machine_len) = self.buffer_variables();
        let offset = BUFFER_OFFSET + evm_state_len + evm_machine_len;

        let data_len: usize = {
            let mut data = self.account.data.borrow_mut();
            let buffer = &mut data[offset..];

            let mut cursor = std::io::Cursor::new(buffer);
            bincode::serialize_into(&mut cursor, &self.data)?;

            cursor.position().try_into()?
        };

        self.header_mut().data_len = data_len;

        Ok(())
    }

    #[must_use]
    pub fn owner(&self) -> Pubkey {
        self.data.owner
    }

    #[must_use]
    pub fn trx(&self) -> &Transaction {
        &self.data.transaction
    }

    #[must_use]
    pub fn trx_origin(&self) -> Address {
        self.data.origin
    }

    #[must_use]
    pub fn trx_chain_id(&self, backend: &impl AccountStorage) -> u64 {
        self.data
            .transaction
            .chain_id()
            .unwrap_or_else(|| backend.default_chain_id())
    }

    #[must_use]
    pub fn gas_used(&self) -> U256 {
        self.data.gas_used
    }

    #[must_use]
    pub fn gas_available(&self) -> U256 {
        self.trx().gas_limit().saturating_sub(self.gas_used())
    }

    pub fn consume_gas(&mut self, amount: U256, receiver: &mut BalanceAccount) -> Result<()> {
        if amount == U256::ZERO {
            return Ok(());
        }

        let trx_chain_id = self.trx().chain_id().unwrap_or(DEFAULT_CHAIN_ID);
        if receiver.chain_id() != trx_chain_id {
            return Err(Error::GasReceiverInvalidChainId);
        }

        let total_gas_used = self.data.gas_used.saturating_add(amount);
        let gas_limit = self.trx().gas_limit();

        if total_gas_used > gas_limit {
            return Err(Error::OutOfGas(gas_limit, total_gas_used));
        }

        self.data.gas_used = total_gas_used;

        let tokens = amount
            .checked_mul(self.trx().gas_price())
            .ok_or(Error::IntegerOverflow)?;
        receiver.mint(tokens)
    }

    pub fn refund_unused_gas(&mut self, origin: &mut BalanceAccount) -> Result<()> {
        let trx_chain_id = self.trx().chain_id().unwrap_or(DEFAULT_CHAIN_ID);

        assert!(origin.chain_id() == trx_chain_id);
        assert!(origin.address() == self.trx_origin());

        let unused_gas = self.gas_available();
        self.consume_gas(unused_gas, origin)
    }
}
