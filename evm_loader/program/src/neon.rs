//! NEON ELF
#![allow(clippy::use_self,clippy::nursery)]

use const_format::formatcp;

const NEON_CONST_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const NEON_CONST_REVISION: &str = "a972362fe1b6d4bea87ffe2cd3bda854fd80c60d";
use crate::account_data::ACCOUNT_SEED_VERSION;
const NEON_CONST_SEED_VERSION: &str = formatcp!("{:?}", ACCOUNT_SEED_VERSION);
use crate::account_data::ACCOUNT_MAX_SIZE;
const NEON_CONST_ACCOUNT_MAX_SIZE: &str = formatcp!("{:?}", ACCOUNT_MAX_SIZE);


const fn create_byte_array<const SZ: usize>(src: &[u8]) -> [u8; SZ] {
    let mut array: [u8; SZ] = [0; SZ];
    let mut i = 0;
    while i < SZ {
        array[i as usize] = src[i as usize];
        i += 1;
    }
    array
}


const fn size_of_byte_array(array: &[u8]) -> usize {
    array.len()
}


const SZ_NEON_PKG_VERSION : usize = size_of_byte_array(NEON_CONST_PKG_VERSION.as_bytes());
/// NEON VERSION
#[no_mangle]
#[used]
pub static NEON_PKG_VERSION: [u8; SZ_NEON_PKG_VERSION] = create_byte_array::<SZ_NEON_PKG_VERSION>(NEON_CONST_PKG_VERSION.as_bytes());


const SZ_NEON_REVISION : usize = size_of_byte_array(NEON_CONST_REVISION.as_bytes());
/// NEON REVISION
#[no_mangle]
#[used]
pub static NEON_REVISION: [u8; SZ_NEON_REVISION] = create_byte_array::<SZ_NEON_REVISION>(NEON_CONST_REVISION.as_bytes());


const SZ_NEON_SEED_VERSION : usize = size_of_byte_array(NEON_CONST_SEED_VERSION.as_bytes());
/// NEON SEED VERSION
#[no_mangle]
#[used]
pub static NEON_SEED_VERSION: [u8; SZ_NEON_SEED_VERSION] = create_byte_array::<SZ_NEON_SEED_VERSION>(NEON_CONST_SEED_VERSION.as_bytes());


const SZ_NEON_ACCOUNT_MAX_SIZE : usize = size_of_byte_array(NEON_CONST_ACCOUNT_MAX_SIZE.as_bytes());
/// NEON ACCOUNT MAX SIZE
#[no_mangle]
#[used]
pub static NEON_ACCOUNT_MAX_SIZE: [u8; SZ_NEON_ACCOUNT_MAX_SIZE] = create_byte_array::<SZ_NEON_ACCOUNT_MAX_SIZE>(NEON_CONST_ACCOUNT_MAX_SIZE.as_bytes());
