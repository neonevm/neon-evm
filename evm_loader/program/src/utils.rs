use primitive_types::{H160, H256, U256};
use solana_program::pubkey::Pubkey;
use solana_program::keccak::{hash, hashv};

pub fn keccak256_h256(data: &[u8]) -> H256 {
    // H256::from_slice(hash(&data).to_bytes().as_slice())
    H256::from(hash(&data).to_bytes())
}

pub fn keccak256_h256_v(data: &[&[u8]]) -> H256 {
    // H256::from_slice(hash(&data).to_bytes().as_slice())
    H256::from(hashv(&data).to_bytes())
}

pub fn keccak256_digest(data: &[u8]) -> Vec<u8> {
    hash(&data).to_bytes().to_vec()
}

pub fn u256_to_h256(value: U256) -> H256 {
    let mut v = vec![0u8; 32];
    value.to_big_endian(&mut v);
    H256::from_slice(&v)
}

pub fn solidity_address(key: &Pubkey) -> H160 {
    H256::from_slice(key.as_ref()).into()
}