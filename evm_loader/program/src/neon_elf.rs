//! NEON ELF
#![allow(clippy::use_self,clippy::nursery)]

use const_format::formatcp;
use crate::macrorules::{ str_as_bytes_len, neon_elf_param };
use crate::config::token_mint::DECIMALS;
use crate::account_data::ACCOUNT_SEED_VERSION;
use crate::account_data::ACCOUNT_MAX_SIZE;

neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_ACCOUNT_MAX_SIZE      , formatcp!("{:?}", ACCOUNT_MAX_SIZE));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", DECIMALS));
