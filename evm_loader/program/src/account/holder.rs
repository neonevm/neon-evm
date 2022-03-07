use std::cell::Ref;
use std::convert::{TryFrom, TryInto};
use crate::account::Operator;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use crate::account;

pub struct Holder<'a> {
    info: &'a AccountInfo<'a>,
}

impl<'a> Holder<'a> {
    pub fn from_account(program_id: &Pubkey, id: u64, info: &'a AccountInfo<'a>, operator: &Operator) -> Result<Self, ProgramError> {
        // WTF!?
        let bytes_count = std::mem::size_of_val(&id);
        let bits_count = bytes_count * 8;
        let holder_id_bit_length = bits_count - id.leading_zeros() as usize;
        let significant_bytes_count = (holder_id_bit_length + 7) / 8;
        let mut hasher = solana_program::keccak::Hasher::default();
        hasher.hash(b"holder");
        hasher.hash(&id.to_be_bytes()[bytes_count - significant_bytes_count..]);
        let output = hasher.result();
        let seed = &hex::encode(output)[..32];

        let expected_key = Pubkey::create_with_seed(operator.key, seed, program_id)?;
        if *info.key != expected_key {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected holder key {}", info.key, expected_key);
        }

        Self::from_account_unchecked(program_id, info)
    }

    pub fn from_account_unchecked(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if account::tag(program_id, info)? != account::TAG_EMPTY {
            return Err!(ProgramError::InvalidAccountData; "Account {} - expected empty tag", info.key)
        }

        Ok(Self { info })
    }

    pub fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ProgramError> {
        let mut data = self.info.try_borrow_mut_data()?;
        let begin = 1_usize/*TAG_EMPTY*/ + offset as usize;
        let end = begin.checked_add(bytes.len())
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account data index overflow"))?;

        if data.len() < end {
            return Err!(ProgramError::AccountDataTooSmall; "Account data too small data.len()={:?}, offset={:?}, bytes.len()={:?}", data.len(), offset, bytes.len());
        }
        data[begin..end].copy_from_slice(bytes);

        Ok(())
    }

    #[must_use]
    pub fn transaction_and_signature(&self) -> (Ref<'a, [u8]>, Ref<'a, [u8; 65]>) {
        fn split_ref_at(origin: Ref<[u8]>, at: usize) -> (Ref<[u8]>, Ref<[u8]>) {
            Ref::map_split(origin, |d| d.split_at(at))
        }

        let data = Ref::map(self.info.data.borrow(), |d| *d);
        let (_tag, rest) = split_ref_at(data, 1);
        let (signature, rest) = split_ref_at(rest, 65);
        let signature = Ref::map(signature, |s| s.try_into().expect("s.len() == 65"));

        let (trx_len, rest) = split_ref_at(rest, 8);
        let trx_len = (*trx_len).try_into().ok().map(u64::from_le_bytes).expect("trx_len is 8 bytes");
        let trx_len = usize::try_from(trx_len).expect("usize is 8 bytes");

        let (trx, _) = split_ref_at(rest, trx_len);

        (trx, signature)
    }
}
