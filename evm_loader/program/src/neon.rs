//! NEON ELF
#![allow(clippy::use_self,clippy::nursery)]

use const_format::formatcp;
use crate::account_data::ACCOUNT_SEED_VERSION;
use crate::account_data::ACCOUNT_MAX_SIZE;
use crate::token::token_mint::TOKEN_MINT_ID;
use crate::payment::collateral_pool_base::COLLATERAL_POOL_BASE;

macro_rules! str_as_bytes_len {
    ($value:expr) => {
        {
            $value.as_bytes().len()
        }
    }
}

macro_rules! neon_elf_params {
    ($identifier:ident,$value:expr) => {
        /// NEON DOCS MUST BE HERE
        #[no_mangle]
        #[used]
        pub static $identifier: [u8; str_as_bytes_len!($value)] = 
            {
                let mut array: [u8; str_as_bytes_len!($value)] = [0; str_as_bytes_len!($value)];
                let mut i = 0;
                while i < str_as_bytes_len!($value) {
                    array[i] = $value.as_bytes()[i];
                    i += 1;
                }
                array
            };
    }
}

neon_elf_params!( NEON_PKG_VERSION          , env!("CARGO_PKG_VERSION"));
neon_elf_params!( NEON_REVISION             , env!("NEON_REVISION"));
neon_elf_params!( NEON_SEED_VERSION         , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_params!( NEON_ACCOUNT_MAX_SIZE     , formatcp!("{:?}", ACCOUNT_MAX_SIZE));
neon_elf_params!( NEON_TOKEN_MINT           , formatcp!("{:?}", TOKEN_MINT_ID));
neon_elf_params!( NEON_POOL_BASE            , formatcp!("{:?}", COLLATERAL_POOL_BASE));
