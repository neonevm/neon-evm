use std::convert::TryInto;

use evm::{U256, H160};
use solana_program::{
    sysvar::Sysvar, 
    rent::Rent,
    program_error::ProgramError,
};
use crate::{
    config::{HOLDER_MSG_SIZE, PAYMENT_TO_TREASURE, STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT},
    account_storage::AccountStorage,
    transaction::Transaction, 
    account::{EthereumAccount, EthereumStorage}
};

use super::ExecutorState;

const LAMPORTS_PER_SIGNATURE: u64 = 5000;

const CREATE_ACCOUNT_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
const WRITE_TO_HOLDER_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
const CANCEL_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
const LAST_ITERATION_COST: u64 = LAMPORTS_PER_SIGNATURE;

const EVM_STEPS_MIN: u64 = 500;
const EVM_STEP_COST: u64 = (LAMPORTS_PER_SIGNATURE / EVM_STEPS_MIN) + (PAYMENT_TO_TREASURE / EVM_STEPS_MIN);

pub struct Gasometer {
    paid_gas: U256,
    gas: u64,
    rent: Rent
}

impl Gasometer {
    pub fn new(paid_gas: Option<U256>) -> Result<Self, ProgramError> {
        let rent = Rent::get()?;

        Ok( Self { 
            paid_gas: paid_gas.unwrap_or(U256::zero()), 
            gas: 0_u64, 
            rent,
        } )
    }

    #[must_use]
    pub fn used_gas(&self) -> U256 {
        U256::from(self.gas)
    }

    #[must_use]
    pub fn used_gas_total(&self) -> U256 {
        self.paid_gas.saturating_add(U256::from(self.gas))
    }

    pub fn record_iterative_overhead(&mut self) {
        // High chance of last iteration to fail with solana error
        // Consume gas for it in the first iteration
        self.gas = self.gas
            .saturating_add(LAST_ITERATION_COST)
            .saturating_add(CANCEL_TRX_COST);
    }

    pub fn record_transaction_size(&mut self, trx: &Transaction) {
        let overhead = 65/*vrs*/ + 8/*u64 len*/;
        let size = trx.rlp_len.saturating_add(overhead);

        let size: u64 = size.try_into().expect("usize is 8 bytes");
        let cost: u64 = (size / HOLDER_MSG_SIZE)
            .saturating_add(1)
            .saturating_mul(WRITE_TO_HOLDER_TRX_COST);

        self.gas = self.gas.saturating_add(cost);
    }

    pub fn record_evm_steps(&mut self, steps: u64) {
        let cost = steps.saturating_mul(EVM_STEP_COST);

        self.gas = self.gas.saturating_add(cost);
    }

    pub fn pad_evm_steps(&mut self, steps: u64) {
        if steps >= EVM_STEPS_MIN {
            return;
        }

        self.record_evm_steps(EVM_STEPS_MIN - steps);
    }

    pub fn record_storage_write<B>(&mut self, state: &ExecutorState<B>, address: H160, key: U256, value: U256)
    where
        B: AccountStorage
    {
        if key < U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT) {
            return;
        }

        if value.is_zero() {
            return;
        }

        if !state.storage(&address, &key).is_zero() {
            return;
        }

        let rent = self.rent.minimum_balance(EthereumStorage::SIZE);

        self.gas = self.gas.saturating_add(rent);
    }

    pub fn record_deploy<B>(&mut self, state: &ExecutorState<B>, address: H160)
    where
        B: AccountStorage
    {
        let (account_space, contract_space) = state.backend.solana_accounts_space(&address);
        let account_rent = self.rent.minimum_balance(account_space);
        let contract_rent = self.rent.minimum_balance(contract_space);

        self.gas = self.gas
            .saturating_add(account_rent)
            .saturating_add(CREATE_ACCOUNT_TRX_COST)
            .saturating_add(contract_rent)
            .saturating_add(CREATE_ACCOUNT_TRX_COST);
    }

    pub fn record_transfer<B>(&mut self, state: &ExecutorState<B>, target: H160, value: U256)
    where
        B: AccountStorage
    {
        if value.is_zero() {
            return;
        }

        let account_is_empty =
            state.balance(&target).is_zero() &&
            state.nonce(&target).is_zero();

        if !account_is_empty {
            return;
        }

        let account_rent = self.rent.minimum_balance(EthereumAccount::SIZE);

        self.gas = self.gas
            .saturating_add(account_rent)
            .saturating_add(CREATE_ACCOUNT_TRX_COST);
    }

    pub fn record_account_rent(&mut self, data_len: usize)
    {
        let account_rent = self.rent.minimum_balance(data_len);
        self.gas = self.gas.saturating_add(account_rent);
    }

    pub fn record_lamports_used(&mut self, lamports: u64)
    {
        self.gas = self.gas.saturating_add(lamports);
    }

}