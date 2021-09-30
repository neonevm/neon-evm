//! NEON ELF
#![allow(clippy::use_self,clippy::nursery)]

use const_format::formatcp;
use crate::account_data::ACCOUNT_SEED_VERSION;
use crate::account_data::ACCOUNT_MAX_SIZE;

macro_rules! str_as_bytes_len {
    ($value:expr) => {
        {
            $value.as_bytes().len()
        }
    }
}

macro_rules! neon_elf_param {
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

macro_rules! declare_param_id {
    ($identifier:ident,$value:expr) => {
            solana_program::declare_id!($value);
            neon_elf_param!( $identifier, $value);
    }
}

neon_elf_param!( NEON_PKG_VERSION          , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION             , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION         , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_ACCOUNT_MAX_SIZE     , formatcp!("{:?}", ACCOUNT_MAX_SIZE));

/// Collateral pool base address
pub mod collateral_pool_base {

    declare_param_id!(NEON_POOL_BASE, "HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");

    /// `COLLATERAL_SEED_PREFIX`
    pub const PREFIX: &str = "collateral_seed_";
}


/// Token Mint ID
pub mod token_mint {

    declare_param_id!(NEON_TOKEN_MINT, "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT");

    /// Number of base 10 digits to the right of the decimal place
    #[must_use]
    pub const fn decimals() -> u8 { 9 }
}