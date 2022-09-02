use std::convert::{Infallible, TryInto};

use arrayref::{array_ref, array_refs};
use evm::{Capture, ExitReason, U256};
use solana_program::{pubkey::Pubkey, program_error::ProgramError};

use crate::{account_storage::AccountStorage, executor::ExecutorState};


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



#[must_use]
#[allow(clippy::too_many_lines)]
pub fn query_account<B: AccountStorage>(
    input: &[u8],
    state: &mut ExecutorState<B>
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("query_account({})", hex::encode(input));

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

            match cache_account(state, account_address, offset, length) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.cache failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_OWNER_ID => {
            debug_print!("query_account.owner({})", account_address);

            match account_owner(state, account_address) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.owner failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_LENGTH_ID => {
            debug_print!("query_account.length({})", account_address);

            match account_data_length(state, account_address) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.length failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_LAMPORTS_ID => {
            debug_print!("query_account.lamports({})", account_address);

            match account_lamports(state, account_address) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.lamports failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_EXECUTABLE_ID => {
            debug_print!("query_account.executable({})", account_address);

            match account_is_executable(state, account_address) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.executable failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_RENT_EPOCH_ID => {
            debug_print!("query_account.rent_epoch({})", account_address);

            match account_rent_epoch(state, account_address) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
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

            match account_data(state, account_address, offset, length) {
                Ok(value) => {
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), value))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.data failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        _ => {
            debug_print!("query_account UNKNOWN {:?}", method_id);
            Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
        }
    }
}


fn cache_account<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
    offset: usize,
    length: usize
) -> Result<Vec<u8>, ProgramError> {
    state.external_account_partial_cache(account_address, offset, length)?;

    Ok(Vec::new())
}

fn account_owner<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    Ok(account.owner.to_bytes().to_vec())
}

fn account_lamports<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    let lamports: U256 = account.lamports.into(); // pad to 32 bytes
    let mut bytes = vec![0_u8; 32];
    lamports.into_big_endian_fast(&mut bytes);

    Ok(bytes)
}

fn account_rent_epoch<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    let epoch: U256 = account.rent_epoch.into(); // pad to 32 bytes
    let mut bytes = vec![0_u8; 32];
    epoch.into_big_endian_fast(&mut bytes);

    Ok(bytes)
}

fn account_is_executable<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    let is_executable: U256 = if account.executable { U256::one() } else { U256::zero() };
    let mut bytes = vec![0_u8; 32];
    is_executable.into_big_endian_fast(&mut bytes);

    Ok(bytes)
}


fn account_data_length<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    let length: U256 = account.data_total_len.into(); // pad to 32 bytes
    let mut bytes = vec![0_u8; 32];
    length.into_big_endian_fast(&mut bytes);

    Ok(bytes)
}

fn account_data<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
    offset: usize,
    length: usize
) -> Result<Vec<u8>, ProgramError> {
    let account = state.external_account_partial(account_address)?;

    if length == 0 {
        return Err(ProgramError::InvalidArgument);
    }

    if offset < account.data_offset {
        return Err(ProgramError::InvalidArgument);
    }

    if offset.saturating_add(length) > account.data_offset.saturating_add(account.data.len()) {
        return Err(ProgramError::InvalidArgument);
    }

    debug_print!("query_account.data got {} bytes", length);

    let begin = offset - account.data_offset;
    let end = begin + length;
    let data = &account.data[begin..end];

    Ok(data.to_vec())
}