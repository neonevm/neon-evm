use std::{convert::{Infallible, TryInto}, str::FromStr};

use arrayref::{array_ref, array_refs};
use evm::{Capture, ExitReason, H160, U256, ExitRevert, ExitSucceed, H256};
use solana_program::{pubkey::Pubkey, program_error::ProgramError, program_pack::Pack, rent::Rent, sysvar::Sysvar, system_instruction};

use crate::{account_storage::AccountStorage, executor::{ExecutorState, Gasometer}, account::ACCOUNT_SEED_VERSION};

const ERC20_METHOD_DECIMALS_ID: &[u8; 4]       = &[0x31, 0x3c, 0xe5, 0x67];
const ERC20_METHOD_TOTAL_SUPPLY_ID: &[u8; 4]   = &[0x18, 0x16, 0x0d, 0xdd];
const ERC20_METHOD_BALANCE_OF_ID: &[u8; 4]     = &[0x70, 0xa0, 0x82, 0x31];
const ERC20_METHOD_TRANSFER_ID: &[u8; 4]       = &[0xa9, 0x05, 0x9c, 0xbb];
const ERC20_METHOD_TRANSFER_FROM_ID: &[u8; 4]  = &[0x23, 0xb8, 0x72, 0xdd];
const ERC20_METHOD_APPROVE_ID: &[u8; 4]        = &[0x09, 0x5e, 0xa7, 0xb3];
const ERC20_METHOD_ALLOWANCE_ID: &[u8; 4]      = &[0xdd, 0x62, 0xed, 0x3e];
const ERC20_METHOD_APPROVE_SOLANA_ID: &[u8; 4] = &[0x93, 0xe2, 0x93, 0x46];

#[must_use]
pub fn erc20_wrapper<B: AccountStorage>(
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("erc20_wrapper({})", hex::encode(input));

    let (token_mint, rest) = input.split_at(32);
    let token_mint = Pubkey::new(token_mint);

    let (method_id, rest) = rest.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    let result = match method_id {
        ERC20_METHOD_DECIMALS_ID => {
            erc20_decimals(state, token_mint)
        },
        ERC20_METHOD_TOTAL_SUPPLY_ID => {
            erc20_total_supply(state, token_mint)
        },
        ERC20_METHOD_BALANCE_OF_ID => {
            let arguments = array_ref![rest, 0, 32];
            let (_, address) = array_refs!(arguments, 12, 20);

            let address = H160::from_slice(address);

            erc20_balance(state, token_mint, context.address, address)
        },
        ERC20_METHOD_TRANSFER_ID => {
            if state.is_static_context() {
                let revert_message = b"ERC20 transfer is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 64];
            let (_, address, value) = array_refs!(arguments, 12, 20, 32);

            let address = H160::from_slice(address);
            let value = U256::from_big_endian_fast(value);

            erc20_transfer(state, gasometer, token_mint, context.address, context.caller, address, value)
        },
        ERC20_METHOD_TRANSFER_FROM_ID => {
            if state.is_static_context() {
                let revert_message = b"ERC20 transferFrom is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 96];
            let (_, source, _, target, value) = array_refs!(arguments, 12, 20, 12, 20, 32);

            let source = H160::from_slice(source);
            let target = H160::from_slice(target);
            let value = U256::from_big_endian_fast(value);
            
            erc20_transfer_from(state, gasometer, token_mint, context.address, context.caller, source, target, value)
        },
        ERC20_METHOD_APPROVE_ID => {
            if state.is_static_context() {
                let revert_message = b"ERC20 approve is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 64];
            let (_, spender, value) = array_refs!(arguments, 12, 20, 32);

            let spender = H160::from_slice(spender);
            let value = U256::from_big_endian_fast(value);

            erc20_approve(state, token_mint, context.address, context.caller, spender, value)
        },
        ERC20_METHOD_ALLOWANCE_ID => {
            let arguments = array_ref![rest, 0, 64];
            let (_, owner, _, spender) = array_refs!(arguments, 12, 20, 12, 20);

            let owner = H160::from_slice(owner);
            let spender = H160::from_slice(spender);

            erc20_allowance(state, token_mint, context.address, owner, spender)
        },
        ERC20_METHOD_APPROVE_SOLANA_ID => {
            if state.is_static_context() {
                let revert_message = b"ERC20 approveSolana is not allowed in static context".to_vec();
                return Capture::Exit((ExitReason::Revert(ExitRevert::Reverted), revert_message))
            }

            let arguments = array_ref![rest, 0, 64];
            let (spender, _, value) = array_refs!(arguments, 32, 24, 8);

            let spender = Pubkey::new_from_array(*spender);
            let value = u64::from_be_bytes(*value);

            erc20_approve_solana(state, token_mint, context.address, context.caller, spender, value)
        },
        _ => {
            return Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
        }
    };

    result.map_or_else(
        |_| Capture::Exit((ExitRevert::Reverted.into(), b"ERC20 execution reverted".to_vec())),
        |value| Capture::Exit((ExitSucceed::Returned.into(), value))
    )
}


fn erc20_total_supply<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey
) -> Result<Vec<u8>, ProgramError> {
    let mint = get_token_mint(state, mint_address)?;
    let supply = U256::from(mint.supply);

    let mut output = vec![0_u8; 32];
    supply.into_big_endian_fast(&mut output);

    Ok(output)
}

fn erc20_decimals<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey
) -> Result<Vec<u8>, ProgramError> {
    let mint = get_token_mint(state, mint_address)?;
    let decimals = U256::from(mint.decimals);

    let mut output = vec![0_u8; 32];
    decimals.into_big_endian_fast(&mut output);

    Ok(output)
}

fn erc20_balance<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey,
    contract: H160,
    account: H160
) -> Result<Vec<u8>, ProgramError> {
    let (token_address, _) = get_token_address(state.backend.program_id(), &account, &contract, &mint_address);

    let token = get_token_account(state, token_address)?;
    let balance = U256::from(token.amount);

    let mut output = vec![0_u8; 32];
    balance.into_big_endian_fast(&mut output);

    Ok(output)
}

#[allow(clippy::unnecessary_wraps)]
fn erc20_allowance<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey,
    contract: H160,
    owner: H160,
    spender: H160,
) -> Result<Vec<u8>, ProgramError> {
    let storage_slot = get_allowance_storage_slot(mint_address, owner, spender);
    let value = state.storage(&contract, &storage_slot);

    let mut output = vec![0_u8; 32];
    value.into_big_endian_fast(&mut output);

    Ok(output)
}

#[allow(clippy::unnecessary_wraps)]
fn erc20_approve<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey,
    contract: H160,
    owner: H160,
    spender: H160,
    value: U256,
) -> Result<Vec<u8>, ProgramError> {
    let storage_slot = get_allowance_storage_slot(mint_address, owner, spender);
    state.set_storage(contract, storage_slot, value);


    // event Approval(address indexed owner, address indexed spender, uint256 value);
    let event_topics = vec![
        H256::from_str("8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925").unwrap(),
        H256::from(owner),
        H256::from(spender)
    ];

    let mut event_data = vec![0_u8; 32];
    value.into_big_endian_fast(&mut event_data);

    state.log(contract, event_topics, event_data);


    let mut output = vec![0_u8; 32];
    output[31] = 1; // return true

    Ok(output)
}


#[allow(clippy::too_many_arguments)]
fn erc20_transfer_from<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer,
    mint_address: Pubkey,
    contract: H160,
    spender: H160,
    source: H160,
    target: H160,
    value: U256
) -> Result<Vec<u8>, ProgramError> {
    let allowance_storage_slot = get_allowance_storage_slot(mint_address, source, spender);
    let allowance = state.storage(&contract, &allowance_storage_slot);

    if allowance < value {
        return Err(ProgramError::InvalidArgument);
    }

    let remaining_allowance = allowance - value;
    state.set_storage(contract, allowance_storage_slot, remaining_allowance);

    erc20_transfer(state, gasometer, mint_address, contract, source, target, value)
}

fn erc20_transfer<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer,
    mint_address: Pubkey,
    contract: H160,
    source: H160,
    target: H160,
    value: U256
) -> Result<Vec<u8>, ProgramError> {
    if value > U256::from(u64::MAX) {
        return Err(ProgramError::InvalidArgument);
    }

    let (source_token, _) = get_token_address(state.backend.program_id(), &source, &contract, &mint_address);
    let source_token_account = get_token_account(state, source_token)?;
    if source_token_account.amount < value.as_u64() {
        return Err(ProgramError::InvalidArgument);
    }

    let (target_token, target_token_bump_seed) = get_token_address(state.backend.program_id(), &target, &contract, &mint_address);

    // create target account if it not exists
    let target_token_account = state.external_account(target_token)?;
    if solana_program::system_program::check_id(&target_token_account.owner) {
        let space = spl_token::state::Account::LEN;
        gasometer.record_account_rent(space);

        let rent = Rent::get()?;
        let minimum_balance = rent.minimum_balance(space);

        let seeds: Vec<Vec<u8>> = vec![
            vec![ACCOUNT_SEED_VERSION], b"ERC20Balance".to_vec(),
            mint_address.to_bytes().to_vec(), contract.to_fixed_bytes().to_vec(), target.to_fixed_bytes().to_vec(),
            vec![target_token_bump_seed]
        ];

        if target_token_account.lamports > 0 {
            let required_lamports = minimum_balance.saturating_sub(target_token_account.lamports);

            if required_lamports > 0 {
                let transfer = system_instruction::transfer(state.backend.operator(), &target_token, required_lamports);
                state.queue_external_instruction(transfer, vec![]);
            }

            let allocate = system_instruction::allocate(&target_token, space.try_into().unwrap());
            state.queue_external_instruction(allocate, seeds.clone());

            let assign = system_instruction::assign(&target_token, &spl_token::ID);
            state.queue_external_instruction(assign, seeds);
        } else {
            let create_account = system_instruction::create_account(
                state.backend.operator(),
                &target_token,
                minimum_balance,
                space.try_into().unwrap(),
                &spl_token::ID,
            );
            state.queue_external_instruction(create_account, seeds);
        }

        let (target_token_owner, _) = state.backend.solana_address(&target);
        let initialize_account = spl_token::instruction::initialize_account2(
            &spl_token::ID, &target_token, &mint_address, &target_token_owner
        )?;
        state.queue_external_instruction(initialize_account, vec![]);
    }

    // do transfer
    let (source_pubkey, source_bump_seed) = state.backend.solana_address(&source);

    let transfer = spl_token::instruction::transfer(
        &spl_token::ID, &source_token, &target_token, &source_pubkey, &[], value.as_u64()
    )?;
    let transfer_seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        source.to_fixed_bytes().to_vec(),
        vec![source_bump_seed]
    ];
    state.queue_external_instruction(transfer, transfer_seeds);


    // event Transfer(address indexed from, address indexed to, uint256 value);
    let event_topics = vec![
        H256::from_str("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").unwrap(),
        H256::from(source),
        H256::from(target)
    ];

    let mut event_data = vec![0_u8; 32];
    value.into_big_endian_fast(&mut event_data);

    state.log(contract, event_topics, event_data);

    // return true
    let mut output = vec![0_u8; 32];
    output[31] = 1;

    Ok(output)
}


fn erc20_approve_solana<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    mint_address: Pubkey,
    contract: H160,
    source: H160,
    spender: Pubkey,
    value: u64,
) -> Result<Vec<u8>, ProgramError> {
    let (source_token, _) = get_token_address(state.backend.program_id(), &source, &contract, &mint_address);
    let (source_pubkey, source_bump_seed) = state.backend.solana_address(&source);

    let approve = spl_token::instruction::approve(
        &spl_token::ID, &source_token, &spender, &source_pubkey, &[], value
    )?;
    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        source.to_fixed_bytes().to_vec(),
        vec![source_bump_seed]
    ];
    state.queue_external_instruction(approve, seeds);


    // event ApprovalSolana(address indexed owner, bytes32 indexed spender, uint64 value);
    let event_topics = vec![
        H256::from_str("f2d0a01e4c49f3439199c8f8950e366e85c4d1bd845552f6da1009b3bb2c1a70").unwrap(),
        H256::from(source),
        H256::from(spender.to_bytes())
    ];

    let mut event_data = vec![0_u8; 32];
    U256::from(value).into_big_endian_fast(&mut event_data);

    state.log(contract, event_topics, event_data);

    // return true
    let mut output = vec![0_u8; 32];
    output[31] = 1;

    Ok(output)
}

fn get_token_mint<B: AccountStorage>(state: &ExecutorState<B>, address: Pubkey) -> Result<spl_token::state::Mint, ProgramError> {
    let account = state.external_account(address)?;
    if !spl_token::check_id(&account.owner) {
        return Ok(spl_token::state::Mint::default())
    }

    spl_token::state::Mint::unpack_unchecked(&account.data)
}

fn get_token_account<B: AccountStorage>(state: &ExecutorState<B>, address: Pubkey) -> Result<spl_token::state::Account, ProgramError> {
    let account = state.external_account(address)?;
    if !spl_token::check_id(&account.owner) {
        return Ok(spl_token::state::Account::default())
    }

    spl_token::state::Account::unpack_unchecked(&account.data)
}

fn get_token_address(program_id: &Pubkey, owner: &H160, contract: &H160, mint: &Pubkey) -> (Pubkey, u8) {
    let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ERC20Balance", &mint.to_bytes(), contract.as_bytes(), owner.as_bytes()];
    Pubkey::find_program_address(seeds, program_id)
}

fn get_allowance_storage_slot(mint: Pubkey, owner: H160, spender: H160) -> U256 {
    use solana_program::keccak::Hash;
    use solana_program::keccak::hashv;

    // mapping (mint => mapping (owner => mapping (spender => uint256)))v
    let Hash(hash) = hashv(&[
        H256::from(spender).as_bytes(),
        H256::from(owner).as_bytes(),
        &mint.to_bytes(),
        &[0xFF; 32]
    ]);

    U256::from_big_endian_fast(&hash)
}