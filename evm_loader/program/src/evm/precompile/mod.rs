use crate::types::Address;
use crate::evm::{database::Database, Machine};
use crate::error::{Error, Result};

mod ecrecover;
mod sha256;
mod ripemd160;
mod datacopy;
mod big_mod_exp;
mod bn256;
mod blake2_f;

// const _SYSTEM_ACCOUNT_ERC20_WRAPPER: Address    = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
// const SYSTEM_ACCOUNT_QUERY: Address             = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
// const SYSTEM_ACCOUNT_NEON_TOKEN: Address        = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
// const SYSTEM_ACCOUNT_SPL_TOKEN: Address         = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);
// const SYSTEM_ACCOUNT_METAPLEX: Address          = Address([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05]);
const SYSTEM_ACCOUNT_ECRECOVER: Address         = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
const SYSTEM_ACCOUNT_SHA_256: Address           = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
const SYSTEM_ACCOUNT_RIPEMD160: Address         = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
const SYSTEM_ACCOUNT_DATACOPY: Address          = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);
const SYSTEM_ACCOUNT_BIGMODEXP: Address         = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05]);
const SYSTEM_ACCOUNT_BN256_ADD: Address         = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]);
const SYSTEM_ACCOUNT_BN256_SCALAR_MUL: Address  = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]);
const SYSTEM_ACCOUNT_BN256_PAIRING: Address     = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]);
const SYSTEM_ACCOUNT_BLAKE2F: Address           = Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09]);


/// Is precompile address
#[must_use]
pub fn is_precompile_address(address: &Address) -> bool {
        *address == SYSTEM_ACCOUNT_ECRECOVER
        || *address == SYSTEM_ACCOUNT_SHA_256
        || *address == SYSTEM_ACCOUNT_RIPEMD160
        || *address == SYSTEM_ACCOUNT_DATACOPY
        || *address == SYSTEM_ACCOUNT_BIGMODEXP
        || *address == SYSTEM_ACCOUNT_BN256_ADD
        || *address == SYSTEM_ACCOUNT_BN256_SCALAR_MUL
        || *address == SYSTEM_ACCOUNT_BN256_PAIRING
        || *address == SYSTEM_ACCOUNT_BLAKE2F
}

impl<B: Database> Machine<B> {
    #[must_use]
    pub fn precompile(address: &Address, data: &[u8]) -> Option<Result<Vec<u8>>> {
        match *address {
            SYSTEM_ACCOUNT_ECRECOVER => Some(Ok(ecrecover::ecrecover(data))),
            SYSTEM_ACCOUNT_SHA_256 => Some(Ok(sha256::sha256(data))),
            SYSTEM_ACCOUNT_RIPEMD160 => Some(Ok(ripemd160::ripemd160(data))),
            SYSTEM_ACCOUNT_DATACOPY => Some(Ok(datacopy::datacopy(data))),
            SYSTEM_ACCOUNT_BIGMODEXP 
            | SYSTEM_ACCOUNT_BN256_ADD 
            | SYSTEM_ACCOUNT_BN256_SCALAR_MUL 
            | SYSTEM_ACCOUNT_BN256_PAIRING => Some(Err(Error::UnimplementedPrecompile(*address))),
            SYSTEM_ACCOUNT_BLAKE2F => Some(Ok(blake2_f::blake2_f(data))),
            _ => None,
        }
    } 
}