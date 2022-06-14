use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use evm::{H160, U256};
use solana_program::pubkey::Pubkey;
use super::Packable;

/// Storage data account to store execution metainfo between steps for iterative execution
#[derive(Debug)]
pub struct Data {
    /// Ethereum transaction caller address
    pub caller: H160,
    /// Ethereum transaction caller nonce
    pub nonce: u64,
    /// Ethereum transaction gas limit
    pub gas_limit: U256,
    /// Ethereum transaction gas price
    pub gas_price: U256,
    /// Last transaction slot
    pub slot: u64,
    /// Operator public key
    pub operator: Pubkey,
    /// Stored accounts length
    pub accounts_len: usize,
    /// Stored executor data size
    pub executor_data_size: usize,
    /// Stored evm data size
    pub evm_data_size: usize,
    /// Ethereum transaction gas used and paid
    pub gas_used_and_paid: U256,
    /// Number of payments
    pub number_of_payments: u64,
    /// ethereum transaction signature
    pub signature: [u8; 65],
}

/// Storage account data for the finalized transaction state
#[derive(Debug)]
pub struct FinalizedData {
    /// caller of the ethereum transaction
    pub sender: H160,
    /// ethereum transaction signature
    pub signature: [u8; 65],
}

impl Packable for Data {
    /// Storage struct tag
    const TAG: u8 = super::TAG_STATE;
    /// Storage struct serialized size
    const SIZE: usize = 20 + 8 + 32 + 32 + 8 + 32 + 8 + 8 + 8 + 32 + 8 + 65;

    /// Deserialize `Storage` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, Data::SIZE];
        let (
            caller,
            nonce,
            gas_limit,
            gas_price,
            slot,
            operator,
            accounts_len,
            executor_data_size,
            evm_data_size,
            gas_used_and_paid,
            number_of_payments,
            sign,
        ) = array_refs![data, 20, 8, 32, 32, 8, 32, 8, 8, 8, 32, 8, 65];

        Self {
            caller: H160::from(*caller),
            nonce: u64::from_le_bytes(*nonce),
            gas_limit: U256::from_little_endian(gas_limit),
            gas_price: U256::from_little_endian(gas_price),
            slot: u64::from_le_bytes(*slot),
            operator: Pubkey::new_from_array(*operator),
            accounts_len: usize::from_le_bytes(*accounts_len),
            executor_data_size: usize::from_le_bytes(*executor_data_size),
            evm_data_size: usize::from_le_bytes(*evm_data_size),
            gas_used_and_paid: U256::from_little_endian(gas_used_and_paid),
            number_of_payments: u64::from_le_bytes(*number_of_payments),
            signature: *sign,
        }
    }

    /// Serialize `Storage` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (
            caller,
            nonce,
            gas_limit,
            gas_price,
            slot,
            operator,
            accounts_len,
            executor_data_size,
            evm_data_size,
            gas_used_and_paid,
            number_of_payments,
            signature,
        ) = mut_array_refs![data, 20, 8, 32, 32, 8, 32, 8, 8, 8, 32, 8, 65];

        *caller = self.caller.to_fixed_bytes();
        *nonce = self.nonce.to_le_bytes();
        self.gas_limit.to_little_endian(gas_limit);
        self.gas_price.to_little_endian(gas_price);
        *slot = self.slot.to_le_bytes();
        operator.copy_from_slice(self.operator.as_ref());
        *accounts_len = self.accounts_len.to_le_bytes();
        *executor_data_size = self.executor_data_size.to_le_bytes();
        *evm_data_size = self.evm_data_size.to_le_bytes();
        self.gas_used_and_paid.to_little_endian(gas_used_and_paid);
        *number_of_payments = self.number_of_payments.to_le_bytes();
        signature.copy_from_slice(self.signature.as_ref());
    }
}

impl Packable for FinalizedData {
    /// Finalized storage struct tag
    const TAG: u8 = super::TAG_FINALIZED_STATE;
    /// Finalized storage struct serialized size
    const SIZE: usize = 20 + 65;

    /// Deserialize `FinalizedState` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, FinalizedData::SIZE];
        let (sender, signature) = array_refs![data, 20, 65];

        Self {
            sender: H160::from(*sender),
            signature: *signature,
        }
    }

    /// Serialize `FinalizedState` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, FinalizedData::SIZE];
        let (sender, signature) = mut_array_refs![data, 20, 65];

        *sender = self.sender.to_fixed_bytes();
        signature.copy_from_slice(self.signature.as_ref());
    }
}
