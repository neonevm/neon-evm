use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use ethnum::U256;
use solana_program::pubkey::Pubkey;
use super::Packable;
use crate::types::Address;

/// Storage data account to store execution metainfo between steps for iterative execution
#[derive(Debug)]
pub struct Data {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
    /// Ethereum transaction caller address
    pub caller: Address,
    /// Ethereum transaction gas limit
    pub gas_limit: U256,
    /// Ethereum transaction gas price
    pub gas_price: U256,
    /// Ethereum transaction gas used and paid
    pub gas_used: U256,
    /// Operator public key
    pub operator: Pubkey,
    /// Starting slot for this operator
    pub slot: u64,
    /// Stored accounts length
    pub accounts_len: usize,
}

/// Storage account data for the finalized transaction state
#[derive(Debug)]
pub struct FinalizedData {
    pub owner: Pubkey,
    pub transaction_hash: [u8; 32],
}

impl Packable for Data {
    /// Storage struct tag
    const TAG: u8 = super::TAG_STATE;
    /// Storage struct serialized size
    const SIZE: usize = 32 + 32 + 20 + 32 + 32 + 32 + 32 + 8 + 8;

    /// Deserialize `Storage` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, Data::SIZE];
        let (
            owner,
            hash,
            caller,
            gas_limit,
            gas_price,
            gas_used,
            operator,
            slot,
            accounts_len,
        ) = array_refs![data, 32, 32, 20, 32, 32, 32, 32, 8, 8];

        Self {
            owner: Pubkey::new_from_array(*owner),
            transaction_hash: *hash,
            caller: Address::from(*caller),
            gas_limit: U256::from_le_bytes(*gas_limit),
            gas_price: U256::from_le_bytes(*gas_price),
            gas_used: U256::from_le_bytes(*gas_used),
            operator: Pubkey::new_from_array(*operator),
            slot: u64::from_le_bytes(*slot),
            accounts_len: usize::from_le_bytes(*accounts_len),
        }
    }

    /// Serialize `Storage` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (
            owner,
            hash,
            caller,
            gas_limit,
            gas_price,
            gas_used,
            operator,
            slot,
            accounts_len,
        ) = mut_array_refs![data, 32, 32, 20, 32, 32, 32, 32, 8, 8];

        owner.copy_from_slice(self.owner.as_ref());
        hash.copy_from_slice(&self.transaction_hash);
        *caller = self.caller.into();
        *gas_limit = self.gas_limit.to_le_bytes();
        *gas_price = self.gas_price.to_le_bytes();
        *gas_used = self.gas_used.to_le_bytes();
        operator.copy_from_slice(self.operator.as_ref());
        *slot = self.slot.to_le_bytes();
        *accounts_len = self.accounts_len.to_le_bytes();
    }
}

impl Packable for FinalizedData {
    /// Finalized storage struct tag
    const TAG: u8 = super::TAG_FINALIZED_STATE;
    /// Finalized storage struct serialized size
    const SIZE: usize = 32 + 32;

    /// Deserialize `FinalizedState` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, FinalizedData::SIZE];
        let (owner, hash) = array_refs![data, 32, 32];

        Self {
            owner: Pubkey::new_from_array(*owner),
            transaction_hash: *hash
        }
    }

    /// Serialize `FinalizedState` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, FinalizedData::SIZE];
        let (owner, hash) = mut_array_refs![data, 32, 32];

        owner.copy_from_slice(self.owner.as_ref());
        hash.copy_from_slice(&self.transaction_hash);
    }
}
