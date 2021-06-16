use evm::{H160, H256, U256};
use solana_program::pubkey::Pubkey;
use solana_program::keccak::{hash, hashv};
use std::convert::TryFrom;

pub fn keccak256_h256(data: &[u8]) -> H256 {
    H256::from(hash(data).to_bytes())
}

pub fn keccak256_h256_v(data: &[&[u8]]) -> H256 {
    H256::from(hashv(data).to_bytes())
}

pub fn keccak256_digest(data: &[u8]) -> Vec<u8> {
    hash(data).to_bytes().to_vec()
}

pub fn u256_to_h256(value: U256) -> H256 {
    let mut v = vec![0_u8; 32];
    value.to_big_endian(&mut v);
    H256::from_slice(&v)
}

pub fn solidity_address(key: &Pubkey) -> H160 {
    H256::from_slice(key.as_ref()).into()
}

#[derive(Debug)]
pub enum EcrecoverError {
    InvalidDigestLength,
    InvalidSignatureLength,
    InvalidRecoveryId,
    InvalidSignature,
    UnknownError
}

pub struct Secp256k1Pubkey([u8; 64]);

impl Secp256k1Pubkey {
    pub fn new(pubkey_vec: &[u8]) -> Self {
        Self(
            <[u8; 64]>::try_from(<&[u8]>::clone(&pubkey_vec))
                .expect("Slice must be the same length as a Pubkey"),
        )
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        self.0
    }
}

#[cfg(target_arch = "bpf")]
pub fn ecrecover(digest: &[u8], recovery_id: u8, signature: &[u8]) -> Result<Secp256k1Pubkey, EcrecoverError> {

    extern "C" {
        fn sol_ecrecover(hash: *const u8, recovery_id: u64, signature: *const u8, result: *mut u8) -> u64;
    };
    let mut pubkey_buffer = [0u8; 64];
    unsafe {
        let result = sol_ecrecover(
            digest.as_ptr(),
            recovery_id as u64,
            signature.as_ptr(),
            pubkey_buffer.as_mut_ptr(),
        );

        match result {
            0 => Ok(Secp256k1Pubkey::new(&pubkey_buffer)),
            1 => Err(EcrecoverError::InvalidDigestLength),
            2 => Err(EcrecoverError::InvalidRecoveryId),
            3 => Err(EcrecoverError::InvalidSignatureLength),
            4 => Err(EcrecoverError::InvalidSignature),
            _ => Err(EcrecoverError::UnknownError),
        }
    }
}

#[cfg(not(target_arch = "bpf"))]
pub fn ecrecover(digest: &[u8], recovery_id: u8, signature: &[u8]) -> Result<Secp256k1Pubkey, EcrecoverError> {
    let message = secp256k1::Message::parse_slice(digest).map_err(|_| EcrecoverError::InvalidDigestLength)?;
    let recovery_id = secp256k1::RecoveryId::parse(recovery_id).map_err(|_| EcrecoverError::InvalidRecoveryId)?;
    let signature = secp256k1::Signature::parse_slice(signature).map_err(|_| EcrecoverError::InvalidSignatureLength)?;

    let secp256k1_key = secp256k1::recover(&message, &signature, &recovery_id).map_err(|_| EcrecoverError::InvalidSignature)?;
    Ok(Secp256k1Pubkey::new(&secp256k1_key.serialize()[1..65]))
}
