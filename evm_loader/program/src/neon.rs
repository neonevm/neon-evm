//! NEON ELF
#![allow(clippy::use_self)]

const NEON_CONST_VERSION: &str = concat!("PKG_VERSION=", env!("CARGO_PKG_VERSION"));
const NEON_CONST_REVISION: &str = "a972362fe1b6d4bea87ffe2cd3bda854fd80c60d";
use crate::account_data::ACCOUNT_SEED_VERSION;

/// NEON VERSION
#[no_mangle]
#[used]
pub static NEON_VERSION: &str = NEON_CONST_VERSION;

/// NEON REVISION
#[no_mangle]
#[used]
pub static NEON_REVISION: &str = NEON_CONST_REVISION;

/// NEON SEED VERSION
#[no_mangle]
#[used]
pub static NEON_SEED_VERSION: u8 = ACCOUNT_SEED_VERSION;

/// NEON BYTE ARRAY
#[no_mangle]
#[used]
pub static NEON_BYTE_ARRAY: &[u8; 4] = &[0x12, 0x34, 0x56, 0x78];
