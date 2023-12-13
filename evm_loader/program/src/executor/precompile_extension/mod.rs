use crate::{account_storage::AccountStorage, error::Result, evm::Context, types::Address};
use maybe_async::maybe_async;

use super::ExecutorState;

mod metaplex;
mod neon_token;
mod query_account;
mod spl_token;

impl<B: AccountStorage> ExecutorState<'_, B> {
    #[deprecated]
    const _SYSTEM_ACCOUNT_ERC20_WRAPPER: Address = Address([
        0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
    ]);
    const SYSTEM_ACCOUNT_QUERY: Address = Address([
        0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02,
    ]);
    const SYSTEM_ACCOUNT_NEON_TOKEN: Address = Address([
        0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03,
    ]);
    const SYSTEM_ACCOUNT_SPL_TOKEN: Address = Address([
        0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04,
    ]);
    const SYSTEM_ACCOUNT_METAPLEX: Address = Address([
        0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05,
    ]);

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn is_precompile_extension(&self, address: &Address) -> bool {
        *address == Self::SYSTEM_ACCOUNT_QUERY
            || *address == Self::SYSTEM_ACCOUNT_NEON_TOKEN
            || *address == Self::SYSTEM_ACCOUNT_SPL_TOKEN
            || *address == Self::SYSTEM_ACCOUNT_METAPLEX
    }

    #[maybe_async]
    pub async fn call_precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        input: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>> {
        match *address {
            Self::SYSTEM_ACCOUNT_QUERY => {
                Some(self.query_account(address, input, context, is_static).await)
            }
            Self::SYSTEM_ACCOUNT_NEON_TOKEN => {
                Some(self.neon_token(address, input, context, is_static).await)
            }
            Self::SYSTEM_ACCOUNT_SPL_TOKEN => {
                Some(self.spl_token(address, input, context, is_static).await)
            }
            Self::SYSTEM_ACCOUNT_METAPLEX => {
                Some(self.metaplex(address, input, context, is_static).await)
            }
            _ => None,
        }
    }
}
