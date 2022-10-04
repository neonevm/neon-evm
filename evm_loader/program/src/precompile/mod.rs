use std::convert::Infallible;

use evm::{H160, Capture, ExitReason};

use crate::{account_storage::AccountStorage, executor::{ExecutorState, Gasometer}};


mod ecrecover;
mod sha256;
mod ripemd160;
mod datacopy;
mod big_mod_exp;
mod bn256;
mod blake2_f;

mod query_account;
mod neon_token;
mod spl_token;
mod metaplex;

#[deprecated]
const _SYSTEM_ACCOUNT_ERC20_WRAPPER: H160 =     H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);

const SYSTEM_ACCOUNT_QUERY: H160 =             H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
const SYSTEM_ACCOUNT_NEON_TOKEN: H160 =        H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
const SYSTEM_ACCOUNT_SPL_TOKEN: H160 =         H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);
const SYSTEM_ACCOUNT_METAPLEX: H160 =          H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05]);
const SYSTEM_ACCOUNT_ECRECOVER: H160 =         H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
const SYSTEM_ACCOUNT_SHA_256: H160 =           H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
const SYSTEM_ACCOUNT_RIPEMD160: H160 =         H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
const SYSTEM_ACCOUNT_DATACOPY: H160 =          H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x04]);
const SYSTEM_ACCOUNT_BIGMODEXP: H160 =         H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05]);
const SYSTEM_ACCOUNT_BN256_ADD: H160 =         H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06]);
const SYSTEM_ACCOUNT_BN256_SCALAR_MUL: H160 =  H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07]);
const SYSTEM_ACCOUNT_BN256_PAIRING: H160 =     H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08]);
const SYSTEM_ACCOUNT_BLAKE2F: H160 =           H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09]);


/// Is precompile address
#[must_use]
pub fn is_precompile_address(address: &H160) -> bool {
           *address == SYSTEM_ACCOUNT_QUERY
        || *address == SYSTEM_ACCOUNT_NEON_TOKEN
        || *address == SYSTEM_ACCOUNT_SPL_TOKEN
        || *address == SYSTEM_ACCOUNT_METAPLEX
        || *address == SYSTEM_ACCOUNT_ECRECOVER
        || *address == SYSTEM_ACCOUNT_SHA_256
        || *address == SYSTEM_ACCOUNT_RIPEMD160
        || *address == SYSTEM_ACCOUNT_DATACOPY
        || *address == SYSTEM_ACCOUNT_BIGMODEXP
        || *address == SYSTEM_ACCOUNT_BN256_ADD
        || *address == SYSTEM_ACCOUNT_BN256_SCALAR_MUL
        || *address == SYSTEM_ACCOUNT_BN256_PAIRING
        || *address == SYSTEM_ACCOUNT_BLAKE2F
}

type PrecompileResult = Capture<(ExitReason, Vec<u8>), Infallible>;

/// Call a precompile function
#[must_use]
pub fn call_precompile<B: AccountStorage>(
    address: H160,
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer
) -> Option<PrecompileResult> {
    if address == SYSTEM_ACCOUNT_QUERY {
        return Some(query_account::query_account(input, state));
    }
    if address == SYSTEM_ACCOUNT_NEON_TOKEN {
        return Some(neon_token::neon_token(input, context, state, gasometer));
    }
    if address == SYSTEM_ACCOUNT_SPL_TOKEN {
        return Some(spl_token::spl_token(input, context, state, gasometer));
    }
    if address == SYSTEM_ACCOUNT_METAPLEX {
        return Some(metaplex::metaplex(input, context, state, gasometer));
    }
    if address == SYSTEM_ACCOUNT_ECRECOVER {
        return Some(ecrecover::ecrecover(input));
    }
    if address == SYSTEM_ACCOUNT_SHA_256 {
        return Some(sha256::sha256(input));
    }
    if address == SYSTEM_ACCOUNT_RIPEMD160 {
        return Some(ripemd160::ripemd160(input));
    }
    if address == SYSTEM_ACCOUNT_DATACOPY {
        return Some(datacopy::datacopy(input));
    }
    if address == SYSTEM_ACCOUNT_BIGMODEXP {
        return Some(big_mod_exp::big_mod_exp(input));
    }
    if address == SYSTEM_ACCOUNT_BN256_ADD {
        return Some(bn256::bn256_add(input));
    }
    if address == SYSTEM_ACCOUNT_BN256_SCALAR_MUL {
        return Some(bn256::bn256_scalar_mul(input));
    }
    if address == SYSTEM_ACCOUNT_BN256_PAIRING {
        return Some(bn256::bn256_pairing(input));
    }
    if address == SYSTEM_ACCOUNT_BLAKE2F {
        return Some(blake2_f::blake2_f(input));
    }

    None
}
