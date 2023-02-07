use std::convert::{TryInto};

use arrayref::{array_ref, array_refs};
use ethnum::U256;
use solana_program::{pubkey::Pubkey};

use crate::{
    account_storage::AccountStorage, 
    executor::ExecutorState,
    error::{Result, Error}, types::Address,
};


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


pub fn query_account<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    address: &Address,
    input: &[u8],
    context: &crate::evm::Context,
    _is_static: bool,
) -> Result<Vec<u8>> {
    debug_print!("query_account({})", hex::encode(input));

    if context.value != 0 {
        return Err(Error::Custom("Query Account: value != 0".to_string()))
    }

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    let (account_address, rest) = rest.split_at(32);
    let account_address = Pubkey::new(account_address);

    match method_id {
        QUERY_ACCOUNT_METHOD_CACHE_ID => {
            let arguments = array_ref![rest, 0, 64];
            let (offset, length) = array_refs!(arguments, 32, 32);
            let offset = U256::from_be_bytes(*offset).as_usize();
            let length = U256::from_be_bytes(*length).as_usize();

            debug_print!("query_account.cache({}, {}, {})", account_address, offset, length);
            cache_account(state, account_address, offset, length)
        },
        QUERY_ACCOUNT_METHOD_OWNER_ID => {
            debug_print!("query_account.owner({})", account_address);
            account_owner(state, account_address)
        },
        QUERY_ACCOUNT_METHOD_LENGTH_ID => {
            debug_print!("query_account.length({})", account_address);
            account_data_length(state, account_address)
        },
        QUERY_ACCOUNT_METHOD_LAMPORTS_ID => {
            debug_print!("query_account.lamports({})", account_address);
            account_lamports(state, account_address)
        },
        QUERY_ACCOUNT_METHOD_EXECUTABLE_ID => {
            debug_print!("query_account.executable({})", account_address);
            account_is_executable(state, account_address)
        },
        QUERY_ACCOUNT_METHOD_RENT_EPOCH_ID => {
            debug_print!("query_account.rent_epoch({})", account_address);
            account_rent_epoch(state, account_address)
        },
        QUERY_ACCOUNT_METHOD_DATA_ID => {
            let arguments = array_ref![rest, 0, 64];
            let (offset, length) = array_refs!(arguments, 32, 32);
            let offset = U256::from_be_bytes(*offset).as_usize();
            let length = U256::from_be_bytes(*length).as_usize();
            debug_print!("query_account.data({}, {}, {})", account_address, offset, length);
            account_data(state, account_address, offset, length)
        },
        _ => {
            debug_print!("query_account UNKNOWN {:?}", method_id);
            Err(Error::UnknownPrecompileMethodSelector(*address, *method_id))
        }
    }
}


fn cache_account<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
    offset: usize,
    length: usize
) -> Result<Vec<u8>> {
    state.external_account_partial_cache(account_address, offset, length)?;

    Ok(Vec::new())
}

fn account_owner<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    Ok(account.owner.to_bytes().to_vec())
}

fn account_lamports<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    let lamports: U256 = account.lamports.into(); // pad to 32 bytes
    let bytes = lamports.to_be_bytes().to_vec();

    Ok(bytes)
}

fn account_rent_epoch<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    let epoch: U256 = account.rent_epoch.into(); // pad to 32 bytes
    let bytes = epoch.to_be_bytes().to_vec();

    Ok(bytes)
}

fn account_is_executable<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    let is_executable: U256 = if account.executable { U256::ONE } else { U256::ZERO };
    let bytes = is_executable.to_be_bytes().to_vec();

    Ok(bytes)
}


fn account_data_length<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    let length: U256 = (account.data_total_len as u128).into(); // pad to 32 bytes
    let bytes = length.to_be_bytes().to_vec();

    Ok(bytes)
}

fn account_data<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account_address: Pubkey,
    offset: usize,
    length: usize
) -> Result<Vec<u8>> {
    let account = state.external_account_partial(account_address)?;

    if length == 0 {
        return Err(Error::Custom("Query Account: data() - length == 0".to_string()));
    }

    if offset < account.data_offset {
        return Err(Error::Custom("Query Account: data() - out of bounds".to_string()));
    }

    if offset.saturating_add(length) > account.data_offset.saturating_add(account.data.len()) {
        return Err(Error::Custom("Query Account: data() - out of bounds".to_string()));
    }

    debug_print!("query_account.data got {} bytes", length);

    let begin = offset - account.data_offset;
    let end = begin + length;
    let data = &account.data[begin..end];

    Ok(data.to_vec())
}