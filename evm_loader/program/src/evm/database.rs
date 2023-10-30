use super::{Buffer, Context};
use crate::{error::Result, types::Address};
use ethnum::U256;
use maybe_async::maybe_async;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

#[maybe_async(?Send)]
pub trait Database {
    fn chain_id(&self) -> U256;

    async fn nonce(&self, address: &Address) -> Result<u64>;
    fn increment_nonce(&mut self, address: Address) -> Result<()>;

    async fn balance(&self, address: &Address) -> Result<U256>;
    async fn transfer(&mut self, source: Address, target: Address, value: U256) -> Result<()>;

    async fn code_size(&self, address: &Address) -> Result<usize>;
    async fn code_hash(&self, address: &Address) -> Result<[u8; 32]>;
    async fn code(&self, address: &Address) -> Result<Buffer>;
    fn set_code(&mut self, address: Address, code: Buffer) -> Result<()>;
    fn selfdestruct(&mut self, address: Address) -> Result<()>;

    async fn storage(&self, address: &Address, index: &U256) -> Result<[u8; 32]>;
    fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()>;

    async fn block_hash(&self, number: U256) -> Result<[u8; 32]>;
    fn block_number(&self) -> Result<U256>;
    fn block_timestamp(&self) -> Result<U256>;

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
