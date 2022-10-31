use std::convert::TryInto;

use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::{
    program_error::ProgramError,
};
use crate::account::Operator;
use crate::{
    config::{HOLDER_MSG_SIZE},
    types::Transaction, 
};

pub const LAMPORTS_PER_SIGNATURE: u64 = 5000;

const WRITE_TO_HOLDER_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
const CANCEL_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
const LAST_ITERATION_COST: u64 = LAMPORTS_PER_SIGNATURE;


pub struct Gasometer {
    paid_gas: U256,
    gas: u64,
    operator_balance: u64
}

impl Gasometer {
    pub fn new(paid_gas: Option<U256>, operator: &Operator) -> Result<Self, ProgramError> {
        Ok( Self {
            paid_gas: paid_gas.unwrap_or(U256::ZERO), 
            gas: 0_u64,
            operator_balance: operator.lamports()
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

    pub fn record_operator_expenses(&mut self, operator: &Operator) {
        let expenses = self.operator_balance.saturating_sub(operator.lamports());

        self.gas = self.gas.saturating_add(expenses);
    }

    pub fn record_solana_transaction_cost(&mut self) {
        self.gas = self.gas.saturating_add(LAMPORTS_PER_SIGNATURE);
    }

    pub fn record_iterative_overhead(&mut self) {
        // High chance of last iteration to fail with solana error
        // Consume gas for it in the first iteration
        self.gas = self.gas
            .saturating_add(LAST_ITERATION_COST)
            .saturating_add(CANCEL_TRX_COST);
    }

    pub fn record_write_to_holder(&mut self, trx: &Transaction) {
        let size: u64 = trx.rlp_len.try_into().expect("usize is 8 bytes");
        let cost: u64 = (size + (HOLDER_MSG_SIZE - 1)) / HOLDER_MSG_SIZE
            .saturating_mul(WRITE_TO_HOLDER_TRX_COST);

        self.gas = self.gas.saturating_add(cost);
    }

    pub fn record_address_lookup_table(&mut self, accounts: &[AccountInfo]) {
        const MIN_ACCOUNTS_TO_USE_ALT: usize = 30;
        const ACCOUNTS_PER_ALT_EXTEND: usize = 30;

        if accounts.len() < MIN_ACCOUNTS_TO_USE_ALT {
            return;
        }

        let extend_count = (accounts.len() + (ACCOUNTS_PER_ALT_EXTEND - 1) ) / ACCOUNTS_PER_ALT_EXTEND;
        // create_alt + extend_alt + deactivate_alt + close_alt
        let cost = (extend_count + 3) as u64 * LAMPORTS_PER_SIGNATURE;

        self.gas = self.gas.saturating_add(cost);
    }
}
