use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

use crate::error::Result;

#[derive(Debug)]
pub struct LegacyHolderData {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
    pub transaction_len: usize,
}

#[derive(Debug)]
pub struct LegacyFinalizedData {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
}

impl LegacyHolderData {
    /// Holder struct tag
    const TAG: u8 = super::TAG_HOLDER_DEPRECATED;
    /// Holder struct serialized size
    const SIZE: usize = 32 + 32 + 8;

    /// Deserialize `Holder` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        let data = arrayref::array_ref![src, 0, LegacyHolderData::SIZE];
        let (owner, hash, len) = arrayref::array_refs![data, 32, 32, 8];

        Self {
            owner: Pubkey::new_from_array(*owner),
            transaction_hash: *hash,
            transaction_len: usize::from_le_bytes(*len),
        }
    }

    pub fn from_account(program_id: &Pubkey, account: &AccountInfo) -> Result<Self> {
        crate::account::validate_tag(program_id, account, Self::TAG)?;

        let data = account.try_borrow_data()?;
        Ok(Self::unpack(&data[1..]))
    }
}

impl LegacyFinalizedData {
    /// Finalized storage struct tag
    const TAG: u8 = super::TAG_STATE_FINALIZED_DEPRECATED;
    /// Finalized storage struct serialized size
    const SIZE: usize = 32 + 32;

    /// Deserialize `FinalizedState` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = arrayref::array_ref![src, 0, LegacyFinalizedData::SIZE];
        let (owner, hash) = arrayref::array_refs![data, 32, 32];

        Self {
            owner: Pubkey::new_from_array(*owner),
            transaction_hash: *hash,
        }
    }

    pub fn from_account(program_id: &Pubkey, account: &AccountInfo) -> Result<Self> {
        crate::account::validate_tag(program_id, account, Self::TAG)?;

        let data = account.try_borrow_data()?;
        Ok(Self::unpack(&data[1..]))
    }
}
