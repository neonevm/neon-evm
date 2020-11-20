use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use primitive_types::{U256,H160};
use solana_sdk::program_error::ProgramError;
use std::convert::TryInto;

#[derive(Debug,Clone)]
pub enum AccountData {
    Empty,
    Account {
        nonce: U256,
        address: H160,
        code_size: u64,
    },
}

impl AccountData {
    pub fn size() -> usize {61}
    pub fn unpack(src: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        use ProgramError::InvalidAccountData;
        let (&tag, rest) = src.split_first().ok_or(InvalidAccountData)?;
        Ok(match tag {
            0 => (Self::Empty, rest),
            1 => {
                let src = array_ref![rest, 0, 60];
                let (nonce, address, code_size) = array_refs![src, 32, 20, 8];
                (Self::Account {
                        nonce: U256::from_little_endian(&*nonce),
                        address: H160::from_slice(&*address),
                        code_size: u64::from_le_bytes(*code_size),
                }, &rest[1..])
            },
            _ => return Err(InvalidAccountData),
        })
    }

    pub fn pack(&self, dst: &mut [u8]) -> usize {
        match self {
            AccountData::Empty => {dst[0] = 0; 1},
            &AccountData::Account {nonce, address, code_size} => {
                dst[0] = 1;
                let dst = array_mut_ref![dst, 1, 60];
                let (nonce_dst, address_dst, code_size_dst) = 
                        mut_array_refs![dst, 32, 20, 8];
                nonce.to_little_endian(&mut *nonce_dst);
                *address_dst = address.to_fixed_bytes();
                *code_size_dst = code_size.to_le_bytes();
                36
            }
        }
    }
}

