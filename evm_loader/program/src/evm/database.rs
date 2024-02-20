use super::{Buffer, Context};
use crate::{error::Result, types::Address};
use ethnum::U256;
use maybe_async::maybe_async;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey, rent::Rent};

#[maybe_async(?Send)]
pub trait Database {
    fn default_chain_id(&self) -> u64;
    fn is_valid_chain_id(&self, chain_id: u64) -> bool;
    async fn contract_chain_id(&self, address: Address) -> Result<u64>;

    async fn nonce(&self, address: Address, chain_id: u64) -> Result<u64>;
    fn increment_nonce(&mut self, address: Address, chain_id: u64) -> Result<()>;

    async fn balance(&self, address: Address, chain_id: u64) -> Result<U256>;
    async fn transfer(
        &mut self,
        source: Address,
        target: Address,
        chain_id: u64,
        value: U256,
    ) -> Result<()>;
    async fn code_size(&self, address: Address) -> Result<usize>;
    async fn code(&self, address: Address) -> Result<Buffer>;
    fn set_code(&mut self, address: Address, chain_id: u64, code: Vec<u8>) -> Result<()>;
    fn selfdestruct(&mut self, address: Address) -> Result<()>;

    async fn storage(&self, address: Address, index: U256) -> Result<[u8; 32]>;
    fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()>;

    async fn block_hash(&self, number: U256) -> Result<[u8; 32]>;
    fn block_number(&self) -> Result<U256>;
    fn block_timestamp(&self) -> Result<U256>;
    fn rent(&self) -> &Rent;
    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)>;

    async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R;

    fn snapshot(&mut self);
    fn revert_snapshot(&mut self);
    fn commit_snapshot(&mut self);

    async fn precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        data: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>>;
}

/// Provides convenience methods that can be implemented in terms of `Database`.
#[maybe_async(?Send)]
pub trait DatabaseExt {
    /// Returns whether an account exists and is non-empty as specified in
    /// https://eips.ethereum.org/EIPS/eip-161.
    async fn account_exists(&self, address: Address, chain_id: u64) -> Result<bool>;

    /// Returns the code hash for an address as specified in
    /// https://eips.ethereum.org/EIPS/eip-1052.
    async fn code_hash(&self, address: Address, chain_id: u64) -> Result<[u8; 32]>;
}

#[maybe_async(?Send)]
impl<T: Database> DatabaseExt for T {
    async fn account_exists(&self, address: Address, chain_id: u64) -> Result<bool> {
        Ok(self.nonce(address, chain_id).await? > 0 || self.balance(address, chain_id).await? > 0)
    }

    async fn code_hash(&self, address: Address, chain_id: u64) -> Result<[u8; 32]> {
        // The function `Database::code` returns a zero-length buffer if the account exists with
        // zero-length code, but also when the account does not exist. This makes it necessary to
        // also check if the account exists when the returned buffer is empty.
        //
        // We could simplify the implementation by checking if the account exists first, but that
        // would lead to more computation in what we think is the common case where the account
        // exists and contains code.
        let code = self.code(address).await?;
        let bytes_to_hash: Option<&[u8]> = if !code.is_empty() {
            Some(&*code)
        } else if self.account_exists(address, chain_id).await? {
            Some(&[])
        } else {
            None
        };

        Ok(bytes_to_hash.map_or([0; 32], |bytes| {
            solana_program::keccak::hash(bytes).to_bytes()
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct TestDatabaseEntry {
        balance: U256,
        nonce: u64,
        code: Vec<u8>,
    }

    impl TestDatabaseEntry {
        fn empty() -> Self {
            Self {
                balance: U256::from(0u8),
                nonce: 0,
                code: Vec::default(),
            }
        }

        fn without_code() -> Self {
            Self {
                balance: U256::from(1u8),
                nonce: 1,
                code: Vec::default(),
            }
        }

        fn with_code(code: Vec<u8>) -> Self {
            assert!(!code.is_empty());
            Self {
                balance: U256::from_words(0, 1),
                nonce: 1,
                code,
            }
        }
    }

    #[derive(Default)]
    struct TestDatabase(HashMap<Address, TestDatabaseEntry>);

    impl FromIterator<(Address, TestDatabaseEntry)> for TestDatabase {
        fn from_iter<T: IntoIterator<Item = (Address, TestDatabaseEntry)>>(iter: T) -> Self {
            Self(iter.into_iter().collect())
        }
    }

    #[maybe_async(?Send)]
    #[allow(unused_variables)]
    impl Database for TestDatabase {
        fn default_chain_id(&self) -> u64 {
            unimplemented!();
        }

        fn is_valid_chain_id(&self, chain_id: u64) -> bool {
            unimplemented!();
        }

        async fn contract_chain_id(&self, address: Address) -> Result<u64> {
            unimplemented!();
        }

        async fn nonce(&self, address: Address, chain_id: u64) -> Result<u64> {
            Ok(self
                .0
                .get(&address)
                .map(|entry| entry.nonce)
                .unwrap_or_default())
        }

        fn increment_nonce(&mut self, address: Address, chain_id: u64) -> Result<()> {
            unimplemented!();
        }

        async fn balance(&self, address: Address, chain_id: u64) -> Result<U256> {
            Ok(self
                .0
                .get(&address)
                .map(|entry| entry.balance)
                .unwrap_or_default())
        }

        async fn transfer(
            &mut self,
            source: Address,
            target: Address,
            chain_id: u64,
            value: U256,
        ) -> Result<()> {
            unimplemented!();
        }

        async fn code_size(&self, address: Address) -> Result<usize> {
            unimplemented!();
        }

        async fn code(&self, address: Address) -> Result<Buffer> {
            Ok(self
                .0
                .get(&address)
                .map(|entry| Buffer::from_slice(&entry.code))
                .unwrap_or_default())
        }

        fn set_code(&mut self, address: Address, chain_id: u64, code: Vec<u8>) -> Result<()> {
            unimplemented!();
        }

        fn selfdestruct(&mut self, address: Address) -> Result<()> {
            unimplemented!();
        }

        async fn storage(&self, address: Address, index: U256) -> Result<[u8; 32]> {
            unimplemented!();
        }

        fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()> {
            unimplemented!();
        }

        async fn block_hash(&self, number: U256) -> Result<[u8; 32]> {
            unimplemented!();
        }

        fn block_number(&self) -> Result<U256> {
            unimplemented!();
        }

        fn block_timestamp(&self) -> Result<U256> {
            unimplemented!();
        }

        fn rent(&self) -> &Rent {
            unimplemented!();
        }

        fn return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
            unimplemented!();
        }

        async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
        where
            F: FnOnce(&AccountInfo) -> R,
        {
            unimplemented!();
        }

        fn snapshot(&mut self) {
            unimplemented!();
        }

        fn revert_snapshot(&mut self) {
            unimplemented!();
        }

        fn commit_snapshot(&mut self) {
            unimplemented!();
        }

        async fn precompile_extension(
            &mut self,
            context: &Context,
            address: &Address,
            data: &[u8],
            is_static: bool,
        ) -> Option<Result<Vec<u8>>> {
            unimplemented!();
        }
    }

    #[maybe_async]
    async fn code_hash(database_entry: Option<TestDatabaseEntry>) -> [u8; 32] {
        let address = Address::default();
        let database: TestDatabase = database_entry
            .map(|entry| (address, entry))
            .into_iter()
            .collect();
        database
            .code_hash(address, crate::config::DEFAULT_CHAIN_ID)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn code_hash_of_non_existing_account() {
        let actual = code_hash(None).await;
        assert_eq!(actual, [0; 32]);
    }

    #[tokio::test]
    async fn code_hash_of_empty_account() {
        let actual = code_hash(Some(TestDatabaseEntry::empty())).await;
        assert_eq!(actual, [0; 32]);
    }

    #[tokio::test]
    async fn code_hash_of_existing_account_without_code() {
        let actual = code_hash(Some(TestDatabaseEntry::without_code())).await;
        assert_eq!(
            actual,
            [
                197, 210, 70, 1, 134, 247, 35, 60, 146, 126, 125, 178, 220, 199, 3, 192, 229, 0,
                182, 83, 202, 130, 39, 59, 123, 250, 216, 4, 93, 133, 164, 112,
            ]
        );
    }

    #[tokio::test]
    async fn code_hash_of_existing_account_with_code() {
        let actual = code_hash(Some(TestDatabaseEntry::with_code(vec![0; 10]))).await;
        assert_eq!(
            actual,
            [
                107, 210, 221, 107, 212, 8, 203, 238, 51, 66, 147, 88, 191, 36, 253, 198, 70, 18,
                251, 248, 177, 180, 219, 96, 69, 24, 244, 15, 253, 52, 182, 7
            ]
        );
    }
}
