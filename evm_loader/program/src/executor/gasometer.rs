use std::convert::TryInto;

use evm::{U256, H160};
use solana_program::{
    sysvar::Sysvar, 
    rent::Rent,
    program_error::ProgramError,
};
use solana_program::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use crate::{
    config::{HOLDER_MSG_SIZE, PAYMENT_TO_TREASURE, STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT},
    account_storage::AccountStorage,
    transaction::Transaction, 
};
use crate::account_storage::{AccountOperation, AccountsOperations};

use super::ExecutorState;

pub const LAMPORTS_PER_SIGNATURE: u64 = 5000;

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
        if key < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            return;
        }

        if value.is_zero() {
            return;
        }

        if !state.storage(&address, &key).is_zero() {
            return;
        }

        let data_len = 1/*tag*/ + 1/*subindex*/ + 32/*value*/;
        self.record_account_rent(data_len);
    }

    pub fn record_accounts_operations_for_emulation(
        &mut self,
        accounts_operations: &AccountsOperations,
    ) {
        for (_address, operation) in accounts_operations {
            match operation {
                AccountOperation::Create { space } => self.record_account_rent(*space),

                AccountOperation::Resize { from, to } => {
                    self.record_account_rent_diff(*from, *to);
                }
            }
        }
    }

    pub fn record_accounts_operations(&mut self, accounts_operations: &AccountsOperations) {
        for (_address, operation) in accounts_operations {
            match operation {
                AccountOperation::Create { space } => self.record_account_rent(
                    (*space).min(MAX_PERMITTED_DATA_INCREASE),
                ),

                AccountOperation::Resize { from, to } => {
                    self.record_account_rent_diff(
                        *from,
                        (*to).min(from.saturating_add(MAX_PERMITTED_DATA_INCREASE)),
                    );
                }
            }
        }
    }

    pub fn record_additional_resize_iterations(&mut self, iteration_count: usize) {
        let cost = (PAYMENT_TO_TREASURE + LAMPORTS_PER_SIGNATURE).saturating_mul(
            iteration_count as u64,
        );
        self.gas = self.gas.saturating_add(cost);
    }

    pub fn record_account_rent(&mut self, data_len: usize) {
        let account_rent = self.rent.minimum_balance(data_len);
        self.gas = self.gas.saturating_add(account_rent);
    }

    pub fn record_account_rent_diff(&mut self, data_len_old: usize, data_len_new: usize) {
        assert!(data_len_new >= data_len_old);
        let account_rent_old = self.rent.minimum_balance(data_len_old);
        let account_rent_new = self.rent.minimum_balance(data_len_new);
        self.gas = self.gas.saturating_add(account_rent_new.saturating_sub(account_rent_old));
    }

    pub fn record_lamports_used(&mut self, lamports: u64)
    {
        self.gas = self.gas.saturating_add(lamports);
    }

    pub fn record_alt_cost(&mut self, alt_cost: u64) { self.gas = self.gas.saturating_add(alt_cost); }
}
