use std::convert::{Infallible, TryInto};

use arrayref::{array_ref, array_refs};
use evm::{Capture, ExitReason, U256};
use solana_program::{pubkey::Pubkey};

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
    debug_print!("query_account({})", hex::encode(&input));

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    let (account_address, rest) = rest.split_at(32);
    let account_address = Pubkey::new(account_address);

    match method_id {
        QUERY_ACCOUNT_METHOD_CACHE_ID => { // Deprecated. Do nothing
            // let arguments = array_ref![rest, 0, 64];
            // let (offset, length) = array_refs!(arguments, 32, 32);
            // let offset = U256::from_big_endian_fast(offset).as_usize();
            // let length = U256::from_big_endian_fast(length).as_usize();
            
            Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![]))
        },
        QUERY_ACCOUNT_METHOD_OWNER_ID => {
            debug_print!("query_account.owner({})", account_address);

            match state.external_account(account_address) {
                Ok(account) => {
                    debug_print!("query_account.owner -> {}", account.owner);
                    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), account.owner.to_bytes().to_vec()))
                }
                Err(err) => {
                    let revert_message = format!("QueryAccount.owner failed: {}", err).as_bytes().to_vec();
                    Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                }
            }
        },
        QUERY_ACCOUNT_METHOD_LENGTH_ID => {
            debug_print!("query_account.length({})", account_address);

            match state.external_account(account_address) {
                Ok(account) => {
                    debug_print!("query_account.length -> {}", account.data.len());
                    let length: U256 = account.data.len().into(); // pad to 32 bytes
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

            match state.external_account(account_address) {
                Ok(account) => {
                    debug_print!("query_account.lamports -> {}", account.lamports);
                    let lamports: U256 = account.lamports.into(); // pad to 32 bytes
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

            match state.external_account(account_address) {
                Ok(account) => {
                    debug_print!("query_account.executable -> {}", account.executable);
                    let executable: U256 = if account.executable { U256::one() } else { U256::zero() }; // pad to 32 bytes
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

            match state.external_account(account_address) {
                Ok(account) => {
                    debug_print!("query_account.rent_epoch -> {}", account.rent_epoch);
                    let rent_epoch: U256 = account.rent_epoch.into(); // pad to 32 bytes
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

            match state.external_account(account_address) {
                Ok(account) => {
                    if offset.saturating_add(length) <= account.data.len() {
                        debug_print!("query_account.data got {} bytes", length);
                        let data = &account.data[offset..offset + length];
                        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), data.to_vec()))
                    } else {
                        let revert_message = b"QueryAccount.data failed".to_vec();
                        Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
                    }
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
