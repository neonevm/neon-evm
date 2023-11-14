use std::convert::TryInto;

use crate::account::Operator;
use crate::{config::HOLDER_MSG_SIZE, types::Transaction};
use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;

pub const LAMPORTS_PER_SIGNATURE: u64 = 5000;

const WRITE_TO_HOLDER_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
pub const CANCEL_TRX_COST: u64 = LAMPORTS_PER_SIGNATURE;
pub const LAST_ITERATION_COST: u64 = LAMPORTS_PER_SIGNATURE;

pub struct Gasometer {
    paid_gas: U256,
    gas: u64,
    refund: u64,
    operator_balance: u64,
}

impl Gasometer {
    pub fn new(paid_gas: U256, operator: &Operator) -> Result<Self, ProgramError> {
        Ok(Self {
            paid_gas,
            gas: 0_u64,
            refund: 0_u64,
            operator_balance: operator.lamports(),
        })
    }

    #[must_use]
    pub fn used_gas(&self) -> U256 {
        U256::from(self.gas.saturating_sub(self.refund))
    }

    #[must_use]
    pub fn used_gas_total(&self) -> U256 {
        self.paid_gas.saturating_add(self.used_gas())
    }

    pub fn refund_lamports(&mut self, lamports: u64) {
        self.refund = self.refund.saturating_add(lamports);
    }

    pub fn record_operator_expenses(&mut self, operator: &Operator) {
        let expenses = self.operator_balance.saturating_sub(operator.lamports());

        self.gas = self.gas.saturating_add(expenses);
    }

    pub fn record_solana_transaction_cost(&mut self) {
        self.gas = self.gas.saturating_add(LAMPORTS_PER_SIGNATURE);
    }

    pub fn record_write_to_holder(&mut self, trx: &Transaction) {
        let size: u64 = trx.rlp_len().try_into().expect("usize is 8 bytes");
        let cost: u64 = ((size + (HOLDER_MSG_SIZE - 1)) / HOLDER_MSG_SIZE)
            .saturating_mul(WRITE_TO_HOLDER_TRX_COST);

        self.gas = self.gas.saturating_add(cost);
    }

    pub fn record_address_lookup_table(&mut self, accounts: &[AccountInfo]) {
        const MIN_ACCOUNTS_TO_USE_ALT: usize = 30;
        const ACCOUNTS_PER_ALT_EXTEND: usize = 30;

        if accounts.len() < MIN_ACCOUNTS_TO_USE_ALT {
            return;
        }

        let extend_count =
            (accounts.len() + (ACCOUNTS_PER_ALT_EXTEND - 1)) / ACCOUNTS_PER_ALT_EXTEND;
        // create_alt + extend_alt + deactivate_alt + close_alt
        let cost = (extend_count + 3) as u64 * LAMPORTS_PER_SIGNATURE;

        self.gas = self.gas.saturating_add(cost);
    }
}
