//! `EVMLoader` precompile contracts

use std::convert::{Infallible, TryInto};
use crate::config::token_mint;
use crate::executor_state::ExecutorState;
use crate::account_storage::AccountStorage;
use crate::utils::keccak256_digest;
use crate::gasometer::Gasometer;

use arrayref::{array_ref, array_refs};
use evm::{Capture, ExitReason, H160, U256};
use solana_program::pubkey::Pubkey;
use solana_program::secp256k1_recover::secp256k1_recover;
use solana_program::alt_bn128::prelude::*;

const SYSTEM_ACCOUNT_ERC20_WRAPPER: H160 =     H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
const SYSTEM_ACCOUNT_QUERY: H160 =             H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
const SYSTEM_ACCOUNT_NEON_TOKEN: H160 =        H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
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
           *address == SYSTEM_ACCOUNT_ERC20_WRAPPER
        || *address == SYSTEM_ACCOUNT_QUERY
        || *address == SYSTEM_ACCOUNT_NEON_TOKEN
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
pub fn call_precompile<'a, B: AccountStorage>(
    address: H160,
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<'a, B>,
    gasometer: &mut Gasometer
) -> Option<PrecompileResult> {
    if address == SYSTEM_ACCOUNT_ERC20_WRAPPER {
        return Some(erc20_wrapper(input, context, state, gasometer));
    }
    if address == SYSTEM_ACCOUNT_QUERY {
        return Some(query_account(input, state));
    }
    if address == SYSTEM_ACCOUNT_NEON_TOKEN {
        return Some(neon_token(input, context, state, gasometer));
    }
    if address == SYSTEM_ACCOUNT_ECRECOVER {
        return Some(ecrecover(input));
    }
    if address == SYSTEM_ACCOUNT_SHA_256 {
        return Some(sha256(input));
    }
    if address == SYSTEM_ACCOUNT_RIPEMD160 {
        return Some(ripemd160(input));
    }
    if address == SYSTEM_ACCOUNT_DATACOPY {
        return Some(datacopy(input));
    }
    if address == SYSTEM_ACCOUNT_BIGMODEXP {
        return Some(big_mod_exp(input, state));
    }
    if address == SYSTEM_ACCOUNT_BN256_ADD {
        return Some(bn256_add(input, state));
    }
    if address == SYSTEM_ACCOUNT_BN256_SCALAR_MUL {
        return Some(bn256_scalar_mul(input, state));
    }
    if address == SYSTEM_ACCOUNT_BN256_PAIRING {
        return Some(bn256_pairing(input, state));
    }
    if address == SYSTEM_ACCOUNT_BLAKE2F {
        return Some(blake2_f(input));
    }

    None
}

// ERC20 method ids:
//--------------------------------------------------
// decimals()                            => 313ce567
// totalSupply()                         => 18160ddd
// balanceOf(address)                    => 70a08231
// transfer(address,uint256)             => a9059cbb
// transferFrom(address,address,uint256) => 23b872dd
// approve(address,uint256)              => 095ea7b3
// allowance(address,address)            => dd62ed3e
// approveSolana(bytes32,uint64)         => 93e29346
//--------------------------------------------------

const ERC20_METHOD_DECIMALS_ID: &[u8; 4]       = &[0x31, 0x3c, 0xe5, 0x67];
const ERC20_METHOD_TOTAL_SUPPLY_ID: &[u8; 4]   = &[0x18, 0x16, 0x0d, 0xdd];
const ERC20_METHOD_BALANCE_OF_ID: &[u8; 4]     = &[0x70, 0xa0, 0x82, 0x31];
const ERC20_METHOD_TRANSFER_ID: &[u8; 4]       = &[0xa9, 0x05, 0x9c, 0xbb];
const ERC20_METHOD_TRANSFER_FROM_ID: &[u8; 4]  = &[0x23, 0xb8, 0x72, 0xdd];
const ERC20_METHOD_APPROVE_ID: &[u8; 4]        = &[0x09, 0x5e, 0xa7, 0xb3];
const ERC20_METHOD_ALLOWANCE_ID: &[u8; 4]      = &[0xdd, 0x62, 0xed, 0x3e];
const ERC20_METHOD_APPROVE_SOLANA_ID: &[u8; 4] = &[0x93, 0xe2, 0x93, 0x46];

/// Call inner `erc20_wrapper`
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn erc20_wrapper<'a, B: AccountStorage>(
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<'a, B>,
    gasometer: &mut Gasometer
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("erc20_wrapper({})", hex::encode(&input));

    let (token_mint, rest) = input.split_at(32);
    let token_mint = Pubkey::new(token_mint);

    let (method_id, rest) = rest.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    match method_id {
        ERC20_METHOD_DECIMALS_ID => {
            debug_print!("erc20_wrapper decimals");
            let supply = state.erc20_decimals(token_mint);
            let supply = U256::from(supply);
            let mut output = vec![0_u8; 32];
            supply.into_big_endian_fast(&mut output);

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_TOTAL_SUPPLY_ID => {
            debug_print!("erc20_wrapper totalSupply");
            let supply = state.erc20_total_supply(token_mint);
            let mut output = vec![0_u8; 32];
            supply.into_big_endian_fast(&mut output);

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_BALANCE_OF_ID => {
            debug_print!("erc20_wrapper balanceOf");

            let arguments = array_ref![rest, 0, 32];
            let (_, address) = array_refs!(arguments, 12, 20);

            let address = H160::from_slice(address);

            let balance = state.erc20_balance_of(token_mint, context, address);
            let mut output = vec![0_u8; 32];
            balance.into_big_endian_fast(&mut output);

            debug_print!("erc20_wrapper balanceOf result {:?}", output);

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_TRANSFER_ID => {
            debug_print!("erc20_wrapper transfer");

            if state.metadata().is_static() {
                let revert_message = b"ERC20 transfer is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }
            let arguments = array_ref![rest, 0, 64];
            let (_, address, value) = array_refs!(arguments, 12, 20, 32);

            let address = H160::from_slice(address);
            let value = U256::from_big_endian_fast(value);

            gasometer.record_spl_transfer(state, address, &token_mint, context);

            let status = state.erc20_transfer(token_mint, context, address, value);
            if !status {
                let revert_message = b"ERC20 transfer failed".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }

            let mut output = vec![0_u8; 32];
            output[31] = 1; // return true

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_TRANSFER_FROM_ID => {
            debug_print!("erc20_wrapper transferFrom");

            if state.metadata().is_static() {
                let revert_message = b"ERC20 transferFrom is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 96];
            let (_, source, _, target, value) = array_refs!(arguments, 12, 20, 12, 20, 32);

            let source = H160::from_slice(source);
            let target = H160::from_slice(target);
            let value = U256::from_big_endian_fast(value);
            gasometer.record_spl_transfer(state, target, &token_mint, context);

            let status = state.erc20_transfer_from(token_mint, context,source, target, value);
            if !status {
                let revert_message = b"ERC20 transferFrom failed".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }

            let mut output = vec![0_u8; 32];
            output[31] = 1; // return true

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_APPROVE_ID => {
            debug_print!("erc20_wrapper approve");

            if state.metadata().is_static() {
                let revert_message = b"ERC20 approve is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 64];
            let (_, spender, value) = array_refs!(arguments, 12, 20, 32);

            let spender = H160::from_slice(spender);
            let value = U256::from_big_endian_fast(value);
            gasometer.record_approve(state, token_mint, context, spender);

            state.erc20_approve(token_mint, context, spender, value);

            let mut output = vec![0_u8; 32];
            output[31] = 1; // return true

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_ALLOWANCE_ID => {
            debug_print!("erc20_wrapper allowance");

            let arguments = array_ref![rest, 0, 64];
            let (_, owner, _, spender) = array_refs!(arguments, 12, 20, 12, 20);

            let owner = H160::from_slice(owner);
            let spender = H160::from_slice(spender);

            let allowance = state.erc20_allowance(token_mint, context, owner, spender);

            let mut output = vec![0_u8; 32];
            allowance.into_big_endian_fast(&mut output);

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        ERC20_METHOD_APPROVE_SOLANA_ID => {
            debug_print!("erc20_wrapper approve solana");

            if state.metadata().is_static() {
                let revert_message = b"ERC20 approveSolana is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 64];
            let (spender, _, value) = array_refs!(arguments, 32, 24, 8);

            let spender = Pubkey::new_from_array(*spender);
            let value = u64::from_be_bytes(*value);

            state.erc20_approve_solana(token_mint, context, spender, value);

            let mut output = vec![0_u8; 32];
            output[31] = 1; // return true

            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output))
        },
        _ => {
            debug_print!("erc20_wrapper UNKNOWN");
            Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
        }
    }
}

// Neon token method ids:
//--------------------------------------------------
// withdraw(bytes32)           => 8e19899e
//--------------------------------------------------
const NEON_TOKEN_METHOD_WITHDRAW_ID: &[u8; 4]       = &[0x8e, 0x19, 0x89, 0x9e];

/// Call inner `neon_token`
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn neon_token<'a, B: AccountStorage>(
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<'a, B>,
    gasometer: &mut Gasometer
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("neon_token({})", hex::encode(&input));

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);
    let min_amount: u64 = u64::pow(10, u32::from(token_mint::decimals()));

    if method_id == NEON_TOKEN_METHOD_WITHDRAW_ID  {
        if state.metadata().is_static() {
            let revert_message = b"neon_token: withdraw is not allowed in static context".to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        let source = context.address; // caller contract

        // owner of the associated token account
        let destination = array_ref![rest, 0, 32];
        let destination = Pubkey::new_from_array(*destination);

        let (spl_amount, remainder) =
            context
            .apparent_value
            .div_mod(U256::from(min_amount));

        if spl_amount > U256::from(u64::MAX) {
            let revert_message = b"neon_token: transfer amount exceeds maximum".to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        if remainder.as_u64() != 0 {
            let revert_message = format!("neon_token: amount must be divisible by {}", min_amount).as_bytes().to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        gasometer.record_withdraw(state, &destination);

        if !state.withdraw(source, destination, context.apparent_value, spl_amount.as_u64()) {
            let revert_message = b"neon_token: failed to withdraw NEON".to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        let mut output = vec![0_u8; 32];
        output[31] = 1; // return true

        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output));
    };

    debug_print!("neon_token UNKNOWN");
    Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
}

// QueryAccount method ids:
//-------------------------------------------
// cache(uint256,uint64,uint64) => 0x2b3c8322
// owner(uint256)               => 0xa123c33e
// length(uint256)              => 0xaa8b99d2
// lamports(uint256)            => 0x748f2d8a
// executable(uint256)          => 0xc219a785
// rent_epoch(uint256)          => 0xc4d369b5
// data(uint256,uint64,uint64)  => 0x43ca5161
//-------------------------------------------

const QUERY_ACCOUNT_METHOD_CACHE_ID: &[u8; 4] = &[0x2b, 0x3c, 0x83, 0x22];
const QUERY_ACCOUNT_METHOD_OWNER_ID: &[u8; 4] = &[0xa1, 0x23, 0xc3, 0x3e];
const QUERY_ACCOUNT_METHOD_LENGTH_ID: &[u8; 4] = &[0xaa, 0x8b, 0x99, 0xd2];
const QUERY_ACCOUNT_METHOD_LAMPORTS_ID: &[u8; 4] = &[0x74, 0x8f, 0x2d, 0x8a];
const QUERY_ACCOUNT_METHOD_EXECUTABLE_ID: &[u8; 4] = &[0xc2, 0x19, 0xa7, 0x85];
const QUERY_ACCOUNT_METHOD_RENT_EPOCH_ID: &[u8; 4] = &[0xc4, 0xd3, 0x69, 0xb5];
const QUERY_ACCOUNT_METHOD_DATA_ID: &[u8; 4] = &[0x43, 0xca, 0x51, 0x61];

/// Call inner `query_account`
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn query_account<'a, B: AccountStorage>(
    input: &[u8],
    state: &mut ExecutorState<'a, B>
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("query_account({})", hex::encode(&input));

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    let (account_address, rest) = rest.split_at(32);
    let account_address = Pubkey::new(account_address);

    match method_id {
        QUERY_ACCOUNT_METHOD_CACHE_ID => {
            let arguments = array_ref![rest, 0, 64];
            let (offset, length) = array_refs!(arguments, 32, 32);
            let offset = U256::from_big_endian_fast(offset).as_usize();
            let length = U256::from_big_endian_fast(length).as_usize();

            debug_print!("query_account.cache({}, {}, {})", account_address, offset, length);
            let r = state.cache_solana_account(account_address, offset, length);
            if r.is_ok() {
                return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![]));
            }

            let revert_message = b"QueryAccount.cache failed".to_vec();
            Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        },
        QUERY_ACCOUNT_METHOD_OWNER_ID => {
            debug_print!("query_account.owner({})", account_address);

            match state.query_solana_account().owner(&account_address) {
                Ok(owner) => {
                    debug_print!("query_account.owner -> {}", owner);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), owner.as_ref().to_owned()))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.owner failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_LENGTH_ID => {
            debug_print!("query_account.length({})", account_address);

            match state.query_solana_account().length(&account_address) {
                Ok(length) => {
                    debug_print!("query_account.length -> {}", length);
                    let length: U256 = length.into(); // pad to 32 bytes
                    let mut bytes = vec![0_u8; 32];
                    length.into_big_endian_fast(&mut bytes);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), bytes))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.length failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_LAMPORTS_ID => {
            debug_print!("query_account.lamports({})", account_address);

            match state.query_solana_account().lamports(&account_address) {
                Ok(lamports) => {
                    debug_print!("query_account.lamports -> {}", lamports);
                    let lamports: U256 = lamports.into(); // pad to 32 bytes
                    let mut bytes = vec![0_u8; 32];
                    lamports.into_big_endian_fast(&mut bytes);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), bytes))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.lamports failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_EXECUTABLE_ID => {
            debug_print!("query_account.executable({})", account_address);

            match state.query_solana_account().executable(&account_address) {
                Ok(executable) => {
                    debug_print!("query_account.executable -> {}", executable);
                    let executable: U256 = if executable { U256::one() } else { U256::zero() }; // pad to 32 bytes
                    let mut bytes = vec![0_u8; 32];
                    executable.into_big_endian_fast(&mut bytes);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), bytes))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.executable failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_RENT_EPOCH_ID => {
            debug_print!("query_account.rent_epoch({})", account_address);

            match state.query_solana_account().rent_epoch(&account_address) {
                Ok(rent_epoch) => {
                    debug_print!("query_account.rent_epoch -> {}", rent_epoch);
                    let rent_epoch: U256 = rent_epoch.into(); // pad to 32 bytes
                    let mut bytes = vec![0_u8; 32];
                    rent_epoch.into_big_endian_fast(&mut bytes);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), bytes))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.rent_epoch failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_DATA_ID => {
            let arguments = array_ref![rest, 0, 64];
            let (offset, length) = array_refs!(arguments, 32, 32);
            let offset = U256::from_big_endian_fast(offset).as_usize();
            let length = U256::from_big_endian_fast(length).as_usize();
            debug_print!("query_account.data({}, {}, {})", account_address, offset, length);

            match state.query_solana_account().data(&account_address, offset, length) {
                Ok(data) => {
                    debug_print!("query_account.data got {} bytes", length);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), data))
                },
                Err(err) => {
                    let revert_message = format!("QueryAccount.data failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                },
            }
        },
        _ => {
            debug_print!("query_account UNKNOWN {:?}", method_id);
            Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
        }
    }
}

/// Call inner `ecrecover`
#[must_use]
pub fn ecrecover(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("ecrecover");
    debug_print!("input: {}", &hex::encode(&input));

    if input.len() != 128 {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32]));
    }

    let data = array_ref![input, 0, 128];
    let (msg, v, sig) = array_refs![data, 32, 32, 64];

    let v: u8 = if let Ok(v) = U256::from(v).as_u32().try_into() {
        v
    } else {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32]));
    };
    let recovery_id = v - 27;
    let public_key = match secp256k1_recover(&msg[..], recovery_id, &sig[..]) {
        Ok(key) => key,
        Err(_) => {
            return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32]))
        }
    };

    let mut address = keccak256_digest(&public_key.to_bytes());
    address[0..12].fill(0);
    debug_print!("{}", &hex::encode(&address));

    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), address))
}

/// Call inner `sha256`
#[must_use]
pub fn sha256(
    input: &[u8],
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    use solana_program::hash::hash as sha256_digest;
    debug_print!("sha256");

    let hash = sha256_digest(input);

    debug_print!("{}", &hex::encode(hash.to_bytes()));

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        hash.to_bytes().to_vec(),
    ))
}

/// Call inner `ripemd160`
#[must_use]
pub fn ripemd160(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    use ripemd160::{Digest, Ripemd160};
    debug_print!("ripemd160");

    let mut hasher = Ripemd160::new();
    // process input message
    hasher.update(input);
    // acquire hash digest in the form of GenericArray,
    // which in this case is equivalent to [u8; 20]
    let hash_val = hasher.finalize();

    // transform to [u8; 32]
    let mut result = vec![0_u8; 12];
    result.extend(&hash_val[..]);

    debug_print!("{}", &hex::encode(&result));

    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), result))
}

/// Call inner datacopy
#[must_use]
pub fn datacopy(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("datacopy");
    debug_print!("input: {}", &hex::encode(&input));

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        input.to_vec(),
    ))
}

/// Call inner `big_mod_exp`
#[must_use]
pub fn big_mod_exp<'a, B: AccountStorage>(
    _input: &[u8],
    _state: &mut ExecutorState<'a, B>
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    // Should be implemented via Solana syscall
    Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![0; 0]))

    /*
    use num_bigint::BigUint;
    use num_traits::{One, Zero};
    debug_print!("big_mod_exp");
    debug_print!("input: {}", &hex::encode(&input));

    if input.len() < 96 {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };

    let (base_len, rest) = input.split_at(32);
    let (exp_len, rest) = rest.split_at(32);
    let (mod_len, rest) = rest.split_at(32);

    let base_len: usize = match U256::from_big_endian(base_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };
    let exp_len: usize = match U256::from_big_endian(exp_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };
    let mod_len: usize = match U256::from_big_endian(mod_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };

    if base_len == 0 && mod_len == 0 {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0_u8; 32]));
    }

    let (base_val, rest) = rest.split_at(base_len);
    let (exp_val, rest) = rest.split_at(exp_len);
    let (mod_val, _rest) = rest.split_at(mod_len);

    let base_val = BigUint::from_bytes_be(base_val);
    let exp_val  = BigUint::from_bytes_be(exp_val);
    let mod_val  = BigUint::from_bytes_be(mod_val);

    if mod_val.is_zero() || mod_val.is_one() {
        let return_value = vec![0_u8; mod_len];
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), return_value));
    }

    let ret_int = base_val.modpow(&exp_val, &mod_val);
    let ret_int = ret_int.to_bytes_be();
    let mut return_value = vec![0_u8; mod_len - ret_int.len()];
    return_value.extend(ret_int);

    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), return_value))
    */
}

/// Call inner `bn256Add`
#[must_use]
#[allow(unused)]
pub fn bn256_add<'a, B: AccountStorage>(
    input: &[u8],
    state: &mut ExecutorState<'a, B>
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("bn256Add");
    debug_print!("input: {}", hex::encode(&input));

    let return_buf = |buf: Vec<u8>| {
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf))
    };
    let mut buf = [0_u8; ALT_BN128_ADDITION_OUTPUT_LEN];

    let result = match alt_bn128_addition(input) {
        Ok(result) => result,
        Err(err) => {
            debug_print!("bn256Add error: {}", err);
            return return_buf(buf.to_vec())
        }
    };
    debug_print!("{}", hex::encode(&result));

    return_buf(result)
}

/// Call inner `bn256ScalarMul`
#[must_use]
#[allow(unused)]
pub fn bn256_scalar_mul<'a, B: AccountStorage>(
    input: &[u8],
    _state: &mut ExecutorState<'a, B>
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("bn256ScalarMul");
    debug_print!("input: {}", &hex::encode(&input));

    let return_buf = |buf: Vec<u8>| {
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf))
    };
    let mut buf = [0_u8; ALT_BN128_MULTIPLICATION_OUTPUT_LEN];

    let result = match alt_bn128_multiplication(input) {
        Ok(result) => result,
        Err(err) => {
            debug_print!("bn256ScalarMul error: {}", err);
            return return_buf(buf.to_vec())
        }
    };
    debug_print!("{}", &hex::encode(&result));

    return_buf(result)
}

/// Call inner `bn256Pairing`
#[must_use]
#[allow(unused)]
pub fn bn256_pairing<'a, B: AccountStorage>(
    input: &[u8],
    _state: &mut ExecutorState<'a, B>
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("bn256Pairing");
    debug_print!("input: {}", &hex::encode(&input));

    let return_buf = |buf: Vec<u8>| {
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf))
    };
    let mut buf = [0_u8; ALT_BN128_PAIRING_OUTPUT_LEN];

    let result = match alt_bn128_pairing(input) {
        Ok(result) => result,
        Err(err) => {
            debug_print!("bn256Pairing error: {}", err);
            return return_buf(buf.to_vec())
        }
    };
    debug_print!("{}", &hex::encode(&result));

    return_buf(result)
}

/// Call inner `blake2F`
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn blake2_f(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    const BLAKE2_F_ARG_LEN: usize = 213;
    debug_print!("blake2F");

    let compress = |h: &mut [u64; 8], m: [u64; 16], t: [u64; 2], f: bool, rounds: usize| {
        const SIGMA: [[usize; 16]; 10] = [
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
            [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
            [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
            [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
            [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
            [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
            [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
            [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
            [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
        ];
        const IV: [u64; 8] = [
            0x6a09_e667_f3bc_c908,
            0xbb67_ae85_84ca_a73b,
            0x3c6e_f372_fe94_f82b,
            0xa54f_f53a_5f1d_36f1,
            0x510e_527f_ade6_82d1,
            0x9b05_688c_2b3e_6c1f,
            0x1f83_d9ab_fb41_bd6b,
            0x5be0_cd19_137e_2179,
        ];
        let g = |v: &mut [u64], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64| {
            v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
            v[d] = (v[d] ^ v[a]).rotate_right(32);
            v[c] = v[c].wrapping_add(v[d]);
            v[b] = (v[b] ^ v[c]).rotate_right(24);
            v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
            v[d] = (v[d] ^ v[a]).rotate_right(16);
            v[c] = v[c].wrapping_add(v[d]);
            v[b] = (v[b] ^ v[c]).rotate_right(63);
        };

        let mut v = [0_u64; 16];
        v[..h.len()].copy_from_slice(h); // First half from state.
        v[h.len()..].copy_from_slice(&IV); // Second half from IV.

        v[12] ^= t[0];
        v[13] ^= t[1];

        if f {
            v[14] = !v[14]; // Invert all bits if the last-block-flag is set.
        }
        for i in 0..rounds {
            // Message word selection permutation for this round.
            let s = &SIGMA[i % 10];
            g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
            g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
            g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
            g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
            g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
            g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
            g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
            g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for i in 0..8 {
            h[i] ^= v[i] ^ v[i + 8];
        }
    };

    if input.len() != BLAKE2_F_ARG_LEN {
        // return Err(ExitError::Other("input length for Blake2 F precompile should be exactly 213 bytes".into()));
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new()));
    }

    let mut rounds_arr: [u8; 4] = Default::default();
    let (rounds_buf, input) = input.split_at(4);
    rounds_arr.copy_from_slice(rounds_buf);
    let rounds: u32 = u32::from_be_bytes(rounds_arr);

    // we use from_le_bytes below to effectively swap byte order to LE if architecture is BE

    let (h_buf, input) = input.split_at(64);
    let mut h = [0_u64; 8];
    let mut ctr = 0;
    for state_word in &mut h {
        let mut temp: [u8; 8] = Default::default();
        temp.copy_from_slice(&h_buf[(ctr * 8)..(ctr + 1) * 8]);
        *state_word = u64::from_le_bytes(temp);
        ctr += 1;
    }

    let (m_buf, input) = input.split_at(128);
    let mut m = [0_u64; 16];
    ctr = 0;
    for msg_word in &mut m {
        let mut temp: [u8; 8] = Default::default();
        temp.copy_from_slice(&m_buf[(ctr * 8)..(ctr + 1) * 8]);
        *msg_word = u64::from_le_bytes(temp);
        ctr += 1;
    }

    let mut t_0_arr: [u8; 8] = Default::default();
    let (t_0_buf, input) = input.split_at(8);
    t_0_arr.copy_from_slice(t_0_buf);
    let t_0 = u64::from_le_bytes(t_0_arr);

    let mut t_1_arr: [u8; 8] = Default::default();
    let (t_1_buf, input) = input.split_at(8);
    t_1_arr.copy_from_slice(t_1_buf);
    let t_1 = u64::from_le_bytes(t_1_arr);

    let f = if input[0] == 1 {
        true
    } else if input[0] == 0 {
        false
    } else {
        // return Err(ExitError::Other("incorrect final block indicator flag".into()))
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new()));
    };

    compress(&mut h, m, [t_0, t_1], f, rounds as usize);

    let mut output_buf = [0_u8; 64];
    for (i, state_word) in h.iter().enumerate() {
        output_buf[i * 8..(i + 1) * 8].copy_from_slice(&state_word.to_le_bytes());
    }

    debug_print!("{}", &hex::encode(&output_buf));

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        output_buf.to_vec(),
    ))
}
