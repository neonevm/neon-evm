//! `EVMLoader` helper functions

use evm::{H256};
use solana_program::keccak::{hash, hashv};

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
