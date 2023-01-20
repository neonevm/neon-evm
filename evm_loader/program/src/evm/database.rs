use ethnum::U256;
use crate::{error::Result, types::Address};
use super::Context;

pub trait Database {
    fn chain_id(&self) -> U256;

    fn nonce(&self, address: &Address) -> Result<u64>;
    fn increment_nonce(&mut self, address: Address) -> Result<()>;

    fn balance(&self, address: &Address) -> Result<U256>;
    fn transfer(&mut self, source: Address, target: Address, value: U256) -> Result<()>;

    fn code_size(&self, address: &Address) -> Result<usize>;
    fn code_hash(&self, address: &Address) -> Result<[u8; 32]>;
    fn code(&self, address: &Address) -> Result<Vec<u8>>;
    fn set_code(&mut self, address: Address, code: Vec<u8>) -> Result<()>;
    fn selfdestruct(&mut self, address: Address) -> Result<()>;

    fn storage(&self, address: &Address, index: &U256) -> Result<[u8; 32]>;
    fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()>;

    fn block_hash(&self, number: U256) -> Result<[u8; 32]>;
    fn block_number(&self) -> Result<U256>;
    fn block_timestamp(&self) -> Result<U256>;

    fn log(&mut self, address: Address, topics: &[[u8; 32]], data: &[u8]) -> Result<()>;

    fn snapshot(&mut self) -> Result<()>;
    fn revert_snapshot(&mut self) -> Result<()>;
    fn commit_snapshot(&mut self) -> Result<()>;

    fn precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        data: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>>;
}
