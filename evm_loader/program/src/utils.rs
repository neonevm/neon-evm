//! `EVMLoader` helper functions

use evm::{H256, U256};
use solana_program::keccak::{hash, hashv};
use crate::config::{
    PAYMENT_TO_TREASURE,
    LAMPORTS_PER_SIGNATURE,
    EVM_STEPS
};

/// Get Keccak256 hash as `H256`
#[must_use]
pub fn keccak256_h256(data: &[u8]) -> H256 {
    H256::from(hash(data).to_bytes())
}

/// Get Keccak256 hash as `H256` from several slices
#[must_use]
pub fn keccak256_h256_v(data: &[&[u8]]) -> H256 {
    H256::from(hashv(data).to_bytes())
}

/// Get Keccak256 hash as Vec<u8>
#[must_use]
pub fn keccak256_digest(data: &[u8]) -> Vec<u8> {
    hash(data).to_bytes().to_vec()
}

/// Convert U256 to H256
#[must_use]
pub fn u256_to_h256(value: U256) -> H256 {
    let mut v = vec![0_u8; 32];
    value.to_big_endian(&mut v);
    H256::from_slice(&v)
}

/// Check whether array is zero initialized
#[must_use]
pub fn is_zero_initialized(data: &[u8]) -> bool {
    for d in data {
        if *d != 0_u8 {
            return false;
        }
    }

    true
}

/// amount of gas per evm-step
#[must_use]
pub fn evm_step_cost(signature_cnt: u64) -> u64 {
    let operator_expences: u64 =  PAYMENT_TO_TREASURE + LAMPORTS_PER_SIGNATURE * signature_cnt;
    operator_expences / EVM_STEPS + u64::from(operator_expences % EVM_STEPS != 0)
}