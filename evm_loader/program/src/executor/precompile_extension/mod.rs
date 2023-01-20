use crate::{
    account_storage::AccountStorage, 
    evm::Context, 
    types::Address,
    error::Result,
};

use super::ExecutorState;


mod neon_token;
mod query_account;
mod spl_token;
mod metaplex;


impl<'a, B: AccountStorage> ExecutorState<'a, B> {
    #[deprecated]
    const _SYSTEM_ACCOUNT_ERC20_WRAPPER: Address = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
    const SYSTEM_ACCOUNT_QUERY: Address          = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
    const SYSTEM_ACCOUNT_NEON_TOKEN: Address     = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
    const SYSTEM_ACCOUNT_SPL_TOKEN: Address      = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);
    const SYSTEM_ACCOUNT_METAPLEX: Address       = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05]);


    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn is_precompile_extension(&self, address: &Address) -> bool {
           *address == Self::SYSTEM_ACCOUNT_QUERY
        || *address == Self::SYSTEM_ACCOUNT_NEON_TOKEN
        || *address == Self::SYSTEM_ACCOUNT_SPL_TOKEN
        || *address == Self::SYSTEM_ACCOUNT_METAPLEX
    }

    pub fn call_precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        input: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>> {
        match *address {
            Self::SYSTEM_ACCOUNT_QUERY => {
                Some(query_account::query_account(self, address, input, context, is_static))
            }
            Self::SYSTEM_ACCOUNT_NEON_TOKEN => {
                Some(neon_token::neon_token(self, address, input, context, is_static))
            }
            Self::SYSTEM_ACCOUNT_SPL_TOKEN => {
                Some(spl_token::spl_token(self, address, input, context, is_static))
            }
            Self::SYSTEM_ACCOUNT_METAPLEX => {
                Some(metaplex::metaplex(self, address, input, context, is_static))
            }
            _ => None
        }
    }
}