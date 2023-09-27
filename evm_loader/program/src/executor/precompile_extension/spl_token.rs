use std::convert::{Into, TryInto};

use ethnum::U256;
use maybe_async::maybe_async;
use solana_program::{
    program_error::ProgramError, program_pack::Pack, pubkey::Pubkey, rent::Rent,
    system_instruction, system_program, sysvar::Sysvar,
};

use crate::{
    account::ACCOUNT_SEED_VERSION,
    account_storage::AccountStorage,
    error::{Error, Result},
    executor::{ExecutorState, OwnedAccountInfo},
    types::Address,
};

// [0xa9, 0xc1, 0x58, 0x06] : "approve(bytes32,bytes32,uint64)",
// [0xc0, 0x67, 0xee, 0xbb] : "burn(bytes32,bytes32,uint64)",
// [0x57, 0x82, 0xa0, 0x43] : "closeAccount(bytes32)",
// [0x6d, 0xa9, 0xde, 0x75] : "isSystemAccount(bytes32)",
// [0xeb, 0x7d, 0xa7, 0x8c] : "findAccount(bytes32)",
// [0x44, 0xef, 0x32, 0x44] : "freeze(bytes32)",
// [0xd1, 0xde, 0x50, 0x11] : "getAccount(bytes32)",
// [0xa2, 0xce, 0x9c, 0x1f] : "getMint(bytes32)",
// [0xda, 0xa1, 0x2c, 0x5c] : "initializeAccount(bytes32,bytes32)",
// [0xfc, 0x86, 0xb7, 0x17] : "initializeAccount(bytes32,bytes32,bytes32)",
// [0xb1, 0x1e, 0xcc, 0x50] : "initializeMint(bytes32,uint8)",
// [0xc3, 0xf3, 0xf2, 0xf2] : "initializeMint(bytes32,uint8,bytes32,bytes32)",
// [0xc9, 0xd0, 0xe2, 0xfd] : "mintTo(bytes32,bytes32,uint64)",
// [0xb7, 0x5c, 0x7d, 0xc6] : "revoke(bytes32)",
// [0x3d, 0x71, 0x8c, 0x9a] : "thaw(bytes32,bytes32)",
// [0x78, 0x42, 0x3b, 0xcf] : "transfer(bytes32,bytes32,uint64)"
// [0x7c, 0x0e, 0xb8, 0x10] : "transferWithSeed(bytes32,bytes32,bytes32,uint64)"

#[allow(clippy::too_many_lines)]
#[maybe_async]
pub async fn spl_token<B: AccountStorage>(
    state: &mut ExecutorState<'_, B>,
    address: &Address,
    input: &[u8],
    context: &crate::evm::Context,
    is_static: bool,
) -> Result<Vec<u8>> {
    if context.value != 0 {
        return Err(Error::Custom("SplToken: value != 0".to_string()));
    }

    if &context.contract != address {
        return Err(Error::Custom(
            "SplToken: callcode or delegatecall is not allowed".to_string(),
        ));
    }

    let (selector, input) = input.split_at(4);
    let selector: [u8; 4] = selector.try_into()?;

    match selector {
        [0xb1, 0x1e, 0xcc, 0x50] => {
            // initializeMint(bytes32 seed, uint8 decimals)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let seed = read_salt(input)?;
            let decimals = read_u8(&input[32..])?;

            initialize_mint(context, state, seed, decimals, None, None).await
        }
        [0xc3, 0xf3, 0xf2, 0xf2] => {
            // initializeMint(bytes32 seed, uint8 decimals, bytes32 mint_authority, bytes32 freeze_authority)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let seed = read_salt(input)?;
            let decimals = read_u8(&input[32..])?;
            let mint_authority = read_pubkey(&input[64..])?;
            let freeze_authority = read_pubkey(&input[96..])?;
            initialize_mint(
                context,
                state,
                seed,
                decimals,
                Some(mint_authority),
                Some(freeze_authority),
            )
            .await
        }
        [0xda, 0xa1, 0x2c, 0x5c] => {
            // initializeAccount(bytes32 seed, bytes32 mint)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let seed = read_salt(input)?;
            let mint = read_pubkey(&input[32..])?;

            initialize_account(context, state, seed, mint, None).await
        }
        [0xfc, 0x86, 0xb7, 0x17] => {
            // initializeAccount(bytes32 seed, bytes32 mint, bytes32 owner)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let seed = read_salt(input)?;
            let mint = read_pubkey(&input[32..])?;
            let owner = read_pubkey(&input[64..])?;
            initialize_account(context, state, seed, mint, Some(owner)).await
        }
        [0x57, 0x82, 0xa0, 0x43] => {
            // closeAccount(bytes32 account)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let account = read_pubkey(input)?;
            close_account(context, state, account)
        }
        [0xa9, 0xc1, 0x58, 0x06] => {
            // approve(bytes32 source, bytes32 target, uint64 amount)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let source = read_pubkey(input)?;
            let target = read_pubkey(&input[32..])?;
            let amount = read_u64(&input[64..])?;
            approve(context, state, source, target, amount)
        }
        [0xb7, 0x5c, 0x7d, 0xc6] => {
            // revoke(bytes32 source)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let source = read_pubkey(input)?;
            revoke(context, state, source)
        }
        [0x78, 0x42, 0x3b, 0xcf] => {
            // transfer(bytes32 source, bytes32 target, uint64 amount)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let source = read_pubkey(input)?;
            let target = read_pubkey(&input[32..])?;
            let amount = read_u64(&input[64..])?;
            transfer(context, state, source, target, amount)
        }
        [0x7c, 0x0e, 0xb8, 0x10] => {
            // transferWithSeed(bytes32,bytes32,bytes32,uint64)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let seed = read_salt(input)?;
            let source = read_pubkey(&input[32..])?;
            let target = read_pubkey(&input[64..])?;
            let amount = read_u64(&input[96..])?;

            transfer_with_seed(context, state, seed, source, target, amount)
        }
        [0xc9, 0xd0, 0xe2, 0xfd] => {
            // mintTo(bytes32 mint, bytes32 account, uint64 amount)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let mint = read_pubkey(input)?;
            let account = read_pubkey(&input[32..])?;
            let amount = read_u64(&input[64..])?;
            mint_to(context, state, mint, account, amount)
        }
        [0xc0, 0x67, 0xee, 0xbb] => {
            // burn(bytes32 mint, bytes32 account, uint64 amount)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let mint = read_pubkey(input)?;
            let account = read_pubkey(&input[32..])?;
            let amount = read_u64(&input[64..])?;
            burn(context, state, mint, account, amount)
        }
        [0x44, 0xef, 0x32, 0x44] => {
            // freeze(bytes32 mint, bytes32 account)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let mint = read_pubkey(input)?;
            let account = read_pubkey(&input[32..])?;
            freeze(context, state, mint, account)
        }
        [0x3d, 0x71, 0x8c, 0x9a] => {
            // thaw(bytes32 mint, bytes32 account)
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let mint = read_pubkey(input)?;
            let account = read_pubkey(&input[32..])?;
            thaw(context, state, mint, account)
        }
        [0xeb, 0x7d, 0xa7, 0x8c] => {
            // findAccount(bytes32 seed)
            let seed = read_salt(input)?;
            find_account(context, state, seed)
        }
        [0x6d, 0xa9, 0xde, 0x75] => {
            // isSystemAccount(bytes32 account)
            let account = read_pubkey(input)?;
            is_system_account(context, state, account).await
        }
        [0xd1, 0xde, 0x50, 0x11] => {
            // getAccount(bytes32 account)
            let account = read_pubkey(input)?;
            get_account(context, state, account).await
        }
        [0xa2, 0xce, 0x9c, 0x1f] => {
            // getMint(bytes32 account)
            let account = read_pubkey(input)?;
            get_mint(context, state, account).await
        }
        _ => Err(Error::UnknownPrecompileMethodSelector(*address, selector)),
    }
}

#[inline]
fn read_u8(input: &[u8]) -> Result<u8> {
    U256::from_be_bytes(*arrayref::array_ref![input, 0, 32])
        .try_into()
        .map_err(Into::into)
}

#[inline]
fn read_u64(input: &[u8]) -> Result<u64> {
    U256::from_be_bytes(*arrayref::array_ref![input, 0, 32])
        .try_into()
        .map_err(Into::into)
}

#[inline]
fn read_pubkey(input: &[u8]) -> Result<Pubkey> {
    if input.len() < 32 {
        return Err(Error::OutOfBounds);
    }
    Ok(Pubkey::new_from_array(*arrayref::array_ref![input, 0, 32]))
}

#[inline]
fn read_salt(input: &[u8]) -> Result<&[u8; 32]> {
    if input.len() < 32 {
        return Err(Error::OutOfBounds);
    }
    Ok(arrayref::array_ref![input, 0, 32])
}

fn create_account<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account: &OwnedAccountInfo,
    space: usize,
    seeds: Vec<Vec<u8>>,
) -> Result<()> {
    let rent = Rent::get()?;
    let minimum_balance = rent.minimum_balance(space);

    let required_lamports = minimum_balance.saturating_sub(account.lamports);

    if required_lamports > 0 {
        let transfer =
            system_instruction::transfer(state.backend.operator(), &account.key, required_lamports);
        state.queue_external_instruction(transfer, vec![], required_lamports);
    }

    let allocate = system_instruction::allocate(&account.key, space.try_into().unwrap());
    state.queue_external_instruction(allocate, seeds.clone(), 0);

    let assign = system_instruction::assign(&account.key, &spl_token::ID);
    state.queue_external_instruction(assign, seeds, 0);

    Ok(())
}

#[maybe_async]
async fn initialize_mint<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<'_, B>,
    seed: &[u8],
    decimals: u8,
    mint_authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, _) = state.backend.solana_address(&signer);

    let (mint_key, bump_seed) = Pubkey::find_program_address(
        &[
            &[ACCOUNT_SEED_VERSION],
            b"ContractData",
            signer.as_bytes(),
            seed,
        ],
        state.backend.program_id(),
    );

    let account = state.external_account(mint_key).await?;
    if !system_program::check_id(&account.owner) {
        return Err(Error::AccountInvalidOwner(mint_key, system_program::ID));
    }

    let seeds: Vec<Vec<u8>> = vec![
        vec![ACCOUNT_SEED_VERSION],
        b"ContractData".to_vec(),
        signer.as_bytes().to_vec(),
        seed.to_vec(),
        vec![bump_seed],
    ];

    create_account(state, &account, spl_token::state::Mint::LEN, seeds)?;

    let initialize_mint = spl_token::instruction::initialize_mint(
        &spl_token::ID,
        &mint_key,
        &mint_authority.unwrap_or(signer_pubkey),
        Some(&freeze_authority.unwrap_or(signer_pubkey)),
        decimals,
    )?;
    state.queue_external_instruction(initialize_mint, vec![], 0);

    Ok(mint_key.to_bytes().to_vec())
}

#[maybe_async]
async fn initialize_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<'_, B>,
    seed: &[u8],
    mint: Pubkey,
    owner: Option<Pubkey>,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, _) = state.backend.solana_address(&signer);

    let (account_key, bump_seed) = Pubkey::find_program_address(
        &[
            &[ACCOUNT_SEED_VERSION],
            b"ContractData",
            signer.as_bytes(),
            seed,
        ],
        state.backend.program_id(),
    );

    let account = state.external_account(account_key).await?;
    if !system_program::check_id(&account.owner) {
        return Err(Error::AccountInvalidOwner(account_key, system_program::ID));
    }

    let seeds: Vec<Vec<u8>> = vec![
        vec![ACCOUNT_SEED_VERSION],
        b"ContractData".to_vec(),
        signer.as_bytes().to_vec(),
        seed.to_vec(),
        vec![bump_seed],
    ];

    create_account(state, &account, spl_token::state::Account::LEN, seeds)?;

    let initialize_mint = spl_token::instruction::initialize_account2(
        &spl_token::ID,
        &account_key,
        &mint,
        &owner.unwrap_or(signer_pubkey),
    )?;
    state.queue_external_instruction(initialize_mint, vec![], 0);

    Ok(account_key.to_bytes().to_vec())
}

fn close_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let close_account = spl_token::instruction::close_account(
        &spl_token::ID,
        &account,
        state.backend.operator(),
        &signer_pubkey,
        &[],
    )?;
    state.queue_external_instruction(close_account, seeds, 0);

    Ok(vec![])
}

fn approve<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    source: Pubkey,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let approve = spl_token::instruction::approve(
        &spl_token::ID,
        &source,
        &target,
        &signer_pubkey,
        &[],
        amount,
    )?;
    state.queue_external_instruction(approve, seeds, 0);

    Ok(vec![])
}

fn revoke<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let revoke = spl_token::instruction::revoke(&spl_token::ID, &account, &signer_pubkey, &[])?;
    state.queue_external_instruction(revoke, seeds, 0);

    Ok(vec![])
}

fn transfer<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    source: Pubkey,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let transfer = spl_token::instruction::transfer(
        &spl_token::ID,
        &source,
        &target,
        &signer_pubkey,
        &[],
        amount,
    )?;
    state.queue_external_instruction(transfer, seeds, 0);

    Ok(vec![])
}

fn transfer_with_seed<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    seed: &[u8; 32],
    source: Pubkey,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>> {
    let seeds: &[&[u8]] = &[
        &[ACCOUNT_SEED_VERSION],
        b"AUTH",
        context.caller.as_bytes(),
        seed,
    ];
    let (signer_pubkey, signer_seed) =
        Pubkey::find_program_address(seeds, state.backend.program_id());

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        b"AUTH".to_vec(),
        context.caller.as_bytes().to_vec(),
        seed.to_vec(),
        vec![signer_seed],
    ];

    let transfer = spl_token::instruction::transfer(
        &spl_token::ID,
        &source,
        &target,
        &signer_pubkey,
        &[],
        amount,
    )?;
    state.queue_external_instruction(transfer, seeds, 0);

    Ok(vec![])
}

fn mint_to<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    mint: Pubkey,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let mint_to = spl_token::instruction::mint_to(
        &spl_token::ID,
        &mint,
        &target,
        &signer_pubkey,
        &[],
        amount,
    )?;
    state.queue_external_instruction(mint_to, seeds, 0);

    Ok(vec![])
}

fn burn<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    mint: Pubkey,
    source: Pubkey,
    amount: u64,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    #[rustfmt::skip]
    let burn = spl_token::instruction::burn(
        &spl_token::ID,
        &source,
        &mint,
        &signer_pubkey,
        &[],
        amount
    )?;
    state.queue_external_instruction(burn, seeds, 0);

    Ok(vec![])
}

fn freeze<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    mint: Pubkey,
    target: Pubkey,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    let freeze = spl_token::instruction::freeze_account(
        &spl_token::ID,
        &target,
        &mint,
        &signer_pubkey,
        &[],
    )?;
    state.queue_external_instruction(freeze, seeds, 0);

    Ok(vec![])
}

fn thaw<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    mint: Pubkey,
    target: Pubkey,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![
        vec![ACCOUNT_SEED_VERSION],
        signer.as_bytes().to_vec(),
        vec![bump_seed],
    ];

    #[rustfmt::skip]
    let thaw = spl_token::instruction::thaw_account(
        &spl_token::ID,
        &target,
        &mint,
        &signer_pubkey,
        &[]
    )?;
    state.queue_external_instruction(thaw, seeds, 0);

    Ok(vec![])
}

#[allow(clippy::unnecessary_wraps)]
fn find_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    seed: &[u8],
) -> Result<Vec<u8>> {
    let signer = context.caller;

    let (account_key, _) = Pubkey::find_program_address(
        &[
            &[ACCOUNT_SEED_VERSION],
            b"ContractData",
            signer.as_bytes(),
            seed,
        ],
        state.backend.program_id(),
    );

    Ok(account_key.to_bytes().to_vec())
}

#[maybe_async]
async fn is_system_account<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<'_, B>,
    account: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account(account).await?;
    if system_program::check_id(&account.owner) {
        let mut result = vec![0_u8; 32];
        result[31] = 1; // return true

        Ok(result)
    } else {
        Ok(vec![0_u8; 32])
    }
}

#[maybe_async]
async fn get_account<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<'_, B>,
    account: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account(account).await?;
    let token = if spl_token::check_id(&account.owner) {
        spl_token::state::Account::unpack(&account.data)?
    } else if system_program::check_id(&account.owner) {
        spl_token::state::Account::default()
    } else {
        return Err(ProgramError::IllegalOwner.into());
    };

    debug_print!("spl_token get_account: {:?}", token);

    let mut result = [0_u8; 7 * 32];
    let (mint, owner, _, amount, delegate, _, delegated_amount, close_authority, state) =
        arrayref::mut_array_refs![&mut result, 32, 32, 24, 8, 32, 24, 8, 32, 32];

    *mint = token.mint.to_bytes();
    *owner = token.owner.to_bytes();
    *amount = token.amount.to_be_bytes();
    *delegate = token.delegate.map(Pubkey::to_bytes).unwrap_or_default();
    *delegated_amount = token.delegated_amount.to_be_bytes();
    *close_authority = token
        .close_authority
        .map(Pubkey::to_bytes)
        .unwrap_or_default();
    state[31] = token.state as u8;

    Ok(result.to_vec())
}

#[maybe_async]
async fn get_mint<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<'_, B>,
    account: Pubkey,
) -> Result<Vec<u8>> {
    let account = state.external_account(account).await?;
    let mint = if spl_token::check_id(&account.owner) {
        spl_token::state::Mint::unpack(&account.data)?
    } else if system_program::check_id(&account.owner) {
        spl_token::state::Mint::default()
    } else {
        return Err(ProgramError::IllegalOwner.into());
    };

    debug_print!("spl_token get_mint: {:?}", mint);

    let mut result = [0_u8; 5 * 32];
    let (_, supply, _, decimals, _, is_initialized, freeze_authority, mint_authority) =
        arrayref::mut_array_refs![&mut result, 24, 8, 31, 1, 31, 1, 32, 32];

    *supply = mint.supply.to_be_bytes();
    *decimals = mint.decimals.to_be_bytes();
    *is_initialized = if mint.is_initialized { [1_u8] } else { [0_u8] };
    *freeze_authority = mint
        .freeze_authority
        .map(Pubkey::to_bytes)
        .unwrap_or_default();
    *mint_authority = mint
        .mint_authority
        .map(Pubkey::to_bytes)
        .unwrap_or_default();

    Ok(result.to_vec())
}
