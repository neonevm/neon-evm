use std::convert::{Infallible};

use evm::{Capture, ExitReason, U256, ExitSucceed, ExitRevert};
use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use mpl_token_metadata::state::{Creator, Metadata, TokenStandard, TokenMetadataAccount};


use crate::{
    account_storage::AccountStorage,
    executor::{ExecutorState, Gasometer}, account::ACCOUNT_SEED_VERSION,
};

// "[0xc5, 0x73, 0x50, 0xc6]": "createMetadata(bytes32,string,string,string)",
// "[0x4a, 0xe8, 0xb6, 0x6b]": "createMasterEdition(bytes32,uint64)",
// "[0x23, 0x5b, 0x2b, 0x94]": "isNFT(bytes32)",
// "[0x9e, 0xd1, 0x9d, 0xdb]": "uri(bytes32)"

#[must_use]
pub fn metaplex<B: AccountStorage>(
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer
) -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    if !context.apparent_value.is_zero() {
        return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), vec![]))
    }

    if context.address == context.caller {
        // callcode is not allowed
        return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), vec![]))
    }

    if context.address != super::SYSTEM_ACCOUNT_METAPLEX {
        // delegatecall is not allowed
        return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), vec![]))
    }


    let (selector, input) = input.split_at(4);
    let result = match *selector {
        [0xc5, 0x73, 0x50, 0xc6] => { // "createMetadata(bytes32,string,string,string)"
            let mint = read_pubkey(input);
            let name = read_string(input, 32);
            let symbol = read_string(input, 64);
            let uri = read_string(input, 96);

            create_metadata(context, state, gasometer, mint, name, symbol, uri)
        }
        [0x4a, 0xe8, 0xb6, 0x6b] => { // "createMasterEdition(bytes32,uint64)"
            let mint = read_pubkey(input);
            let max_supply = read_u64(&input[32..]);

            create_master_edition(context, state, gasometer, mint, Some(max_supply))
        }
        [0x23, 0x5b, 0x2b, 0x94] => { // "isNFT(bytes32)"
            let mint = read_pubkey(input);
            is_nft(context, state, gasometer, mint)
        }
        [0x9e, 0xd1, 0x9d, 0xdb] => { // "uri(bytes32)"
            let mint = read_pubkey(input);
            uri(context, state, gasometer, mint)
        }
        _ => {
            Ok(vec![])
        }
    };


    result.map_or_else(
        |_| Capture::Exit((ExitRevert::Reverted.into(), vec![])),
        |value| Capture::Exit((ExitSucceed::Returned.into(), value))
    )
}


#[inline]
fn read_u64(input: &[u8]) -> u64 {
    U256::from_big_endian_fast(arrayref::array_ref![input, 0, 32]).as_u64()
}

#[inline]
fn read_pubkey(input: &[u8]) -> Pubkey {
    Pubkey::new_from_array(*arrayref::array_ref![input, 0, 32])
}

#[inline]
fn read_string(input: &[u8], offset_position: usize) -> String {
    let offset = U256::from_big_endian_fast(arrayref::array_ref![input, offset_position, 32]).as_usize();
    let length = U256::from_big_endian_fast(arrayref::array_ref![input, offset, 32]).as_usize();

    let begin = offset + 32;
    let end = begin + length;

    let data = input[begin..end].to_vec();
    unsafe { String::from_utf8_unchecked(data) }
}


fn create_metadata<B: AccountStorage>(
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer,
    mint: Pubkey,
    name: String,
    symbol: String,
    uri: String,
) -> Result<Vec<u8>, ProgramError>
{
    if state.is_static_context() {
        return Err!(ProgramError::InvalidArgument; "Action is not allowed in static context")
    }

    gasometer.record_account_rent(mpl_token_metadata::state::MAX_METADATA_LEN);

    
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];
    
    let (metadata_pubkey, _) = mpl_token_metadata::pda::find_metadata_account(&mint);

    let instruction = mpl_token_metadata::instruction::create_metadata_accounts_v3(
        mpl_token_metadata::ID,
        metadata_pubkey,
        mint,
        signer_pubkey,
        *state.backend.operator(),
        signer_pubkey,
        name,
        symbol,
        uri,
        Some(vec![
            Creator { address: *state.backend.program_id(), verified: false, share: 0 },
            Creator { address: signer_pubkey, verified: true, share: 100 }
        ]),
        0,     // Seller Fee
        true,  // Update Authority == Mint Authority
        false, // Is Mutable
        None,  // Collection
        None,  // Uses
        None,  // Collection Details
    );
    state.queue_external_instruction(instruction, seeds);

    Ok(metadata_pubkey.to_bytes().to_vec())
}


fn create_master_edition<B: AccountStorage>(
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer,
    mint: Pubkey,
    max_supply: Option<u64>,
) -> Result<Vec<u8>, ProgramError>
{
    if state.is_static_context() {
        return Err!(ProgramError::InvalidArgument; "Action is not allowed in static context")
    }

    gasometer.record_account_rent(mpl_token_metadata::state::MAX_MASTER_EDITION_LEN);

    
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];
    
    let (metadata_pubkey, _) = mpl_token_metadata::pda::find_metadata_account(&mint);
    let (edition_pubkey, _) = mpl_token_metadata::pda::find_master_edition_account(&mint);

    let instruction = mpl_token_metadata::instruction::create_master_edition_v3(
        mpl_token_metadata::ID,
        edition_pubkey,
        mint,
        signer_pubkey,
        signer_pubkey,
        metadata_pubkey,
        *state.backend.operator(),
        max_supply,
    );
    state.queue_external_instruction(instruction, seeds);

    Ok(edition_pubkey.to_bytes().to_vec())
}


fn is_nft<B: AccountStorage>(
    _context: &evm::Context,
    state: &mut ExecutorState<B>,
    _gasometer: &mut Gasometer,
    mint: Pubkey,
) -> Result<Vec<u8>, ProgramError>
{
    let (metadata_pubkey, _) = mpl_token_metadata::pda::find_metadata_account(&mint);
    let metadata_account = state.external_account(metadata_pubkey)?;

    if !mpl_token_metadata::check_id(&metadata_account.owner) {
        return Ok(vec![0_u8; 32]);
    }

    let metadata: Metadata = match Metadata::safe_deserialize(&metadata_account.data) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(vec![0_u8; 32])
    };

    if metadata.token_standard != Some(TokenStandard::NonFungible) {
        return Ok(vec![0_u8; 32]);
    }


    let mut result = vec![0_u8; 32];
    result[31] = 1; // return true

    Ok(result)
}

fn uri<B: AccountStorage>(
    _context: &evm::Context,
    state: &mut ExecutorState<B>,
    _gasometer: &mut Gasometer,
    mint: Pubkey,
) -> Result<Vec<u8>, ProgramError>
{
    let (metadata_pubkey, _) = mpl_token_metadata::pda::find_metadata_account(&mint);
    let metadata_account = state.external_account(metadata_pubkey)?;

    let uri = {
        if mpl_token_metadata::check_id(&metadata_account.owner) {
            let metadata: Result<Metadata, _> = Metadata::safe_deserialize(&metadata_account.data);
            metadata.map_or_else(|_| String::new(), |m| m.data.uri)
        } else {
            String::new()
        }
    };
    let uri = uri.trim_end_matches('\0');

    // String encoding
    // 32 bytes - offset
    // 32 bytes - length
    // length + padding bytes - data

    let data_len = if uri.len() % 32 == 0 {
        std::cmp::max(uri.len(), 32)
    } else {
        ((uri.len() / 32) + 1) * 32
    };

    let mut result = vec![0_u8; 32 + 32 + data_len];

    result[31] = 0x20; // offset - 32 bytes

    let length = U256::from(uri.len());
    length.into_big_endian_fast(&mut result[32..64]);
    
    result[64..64 + uri.len()].copy_from_slice(uri.as_bytes());


    Ok(result)
}