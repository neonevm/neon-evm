//! `EVMLoader` helper functions
use thiserror::Error;

use evm::{H160, H256, U256};
use solana_program::pubkey::Pubkey;
use solana_program::keccak::{hash, hashv};
use std::convert::TryFrom;

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

/// Get ethereum address from solana `Pubkey`
#[must_use]
pub fn solidity_address(key: &Pubkey) -> H160 {
    H256::from_slice(key.as_ref()).into()
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Secp256k1RecoverError {
    #[error("The hash provided to a secp256k1_recover is invalid")]
    InvalidHash,
    #[error("The recovery_id provided to a secp256k1_recover is invalid")]
    InvalidRecoveryId,
    #[error("The signature provided to a secp256k1_recover is invalid")]
    InvalidSignature,
}

impl From<u64> for Secp256k1RecoverError {
    fn from(v: u64) -> Secp256k1RecoverError {
        match v {
            1 => Secp256k1RecoverError::InvalidHash,
            2 => Secp256k1RecoverError::InvalidRecoveryId,
            3 => Secp256k1RecoverError::InvalidSignature,
            _ => panic!("Unsupported Secp256k1RecoverError"),
        }
    }
}

impl From<Secp256k1RecoverError> for u64 {
    fn from(v: Secp256k1RecoverError) -> u64 {
        match v {
            Secp256k1RecoverError::InvalidHash => 1,
            Secp256k1RecoverError::InvalidRecoveryId => 2,
            Secp256k1RecoverError::InvalidSignature => 3,
        }
    }
}

pub const SECP256K1_SIGNATURE_LENGTH: usize = 64;
pub const SECP256K1_PUBLIC_KEY_LENGTH: usize = 64;

#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub struct Secp256k1Pubkey(pub [u8; SECP256K1_PUBLIC_KEY_LENGTH]);

impl Secp256k1Pubkey {
    pub fn new(pubkey_vec: &[u8]) -> Self {
        Self(
            <[u8; SECP256K1_PUBLIC_KEY_LENGTH]>::try_from(<&[u8]>::clone(&pubkey_vec))
                .expect("Slice must be the same length as a Pubkey"),
        )
    }

    pub fn to_bytes(self) -> [u8; 64] {
        self.0
    }
}

pub fn secp256k1_recover(
    hash: &[u8],
    recovery_id: u8,
    signature: &[u8],
) -> Result<Secp256k1Pubkey, Secp256k1RecoverError> {
    #[cfg(target_arch = "bpf")]
    {
        extern "C" {
            fn sol_secp256k1_recover(
                hash: *const u8,
                recovery_id: u64,
                signature: *const u8,
                result: *mut u8,
            ) -> u64;
        }

        let mut pubkey_buffer = [0u8; SECP256K1_PUBLIC_KEY_LENGTH];
        let result = unsafe {
            sol_secp256k1_recover(
                hash.as_ptr(),
                recovery_id as u64,
                signature.as_ptr(),
                pubkey_buffer.as_mut_ptr(),
            )
        };

        match result {
            0 => Ok(Secp256k1Pubkey::new(&pubkey_buffer)),
            error => Err(Secp256k1RecoverError::from(error)),
        }
    }

    #[cfg(not(target_arch = "bpf"))]
    {
        let message = libsecp256k1::Message::parse_slice(hash)
            .map_err(|_| Secp256k1RecoverError::InvalidHash)?;
        let recovery_id = libsecp256k1::RecoveryId::parse(recovery_id)
            .map_err(|_| Secp256k1RecoverError::InvalidRecoveryId)?;
        let signature = libsecp256k1::Signature::parse_standard_slice(signature)
            .map_err(|_| Secp256k1RecoverError::InvalidSignature)?;

        let secp256k1_key = libsecp256k1::recover(&message, &signature, &recovery_id)
            .map_err(|_| Secp256k1RecoverError::InvalidSignature)?;
        Ok(Secp256k1Pubkey::new(&secp256k1_key.serialize()[1..65]))
    }
}
