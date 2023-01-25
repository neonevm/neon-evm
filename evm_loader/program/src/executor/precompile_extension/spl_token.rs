use std::convert::{TryInto};

use ethnum::U256;
use solana_program::{
    pubkey::Pubkey, rent::Rent, sysvar::Sysvar, 
    system_instruction, program_pack::Pack, system_program
};

use crate::{
    account_storage::AccountStorage,
    executor::{ExecutorState, OwnedAccountInfo}, 
    account::ACCOUNT_SEED_VERSION, 
    types::Address,
    error::{Error, Result}
};

// [0xa9, 0xc1, 0x58, 0x06] : "approve(bytes32,bytes32,uint64)",
// [0xe3, 0x41, 0x08, 0x55] : "burn(bytes32,uint64)",
// [0x57, 0x82, 0xa0, 0x43] : "closeAccount(bytes32)",
// [0x38, 0xa6, 0x99, 0xa4] : "exists(bytes32)",
// [0xeb, 0x7d, 0xa7, 0x8c] : "findAccount(bytes32)",
// [0xec, 0x13, 0xcc, 0x7b] : "freeze(bytes32)",
// [0xd1, 0xde, 0x50, 0x11] : "getAccount(bytes32)",
// [0xa2, 0xce, 0x9c, 0x1f] : "getMint(bytes32)",
// [0xda, 0xa1, 0x2c, 0x5c] : "initializeAccount(bytes32,bytes32)",
// [0xfc, 0x86, 0xb7, 0x17] : "initializeAccount(bytes32,bytes32,bytes32)",
// [0xb1, 0x1e, 0xcc, 0x50] : "initializeMint(bytes32,uint8)",
// [0xc3, 0xf3, 0xf2, 0xf2] : "initializeMint(bytes32,uint8,bytes32,bytes32)",
// [0xa9, 0x05, 0x74, 0x01] : "mintTo(bytes32,uint64)",
// [0xb7, 0x5c, 0x7d, 0xc6] : "revoke(bytes32)",
// [0xc2, 0x59, 0xdd, 0xfe] : "thaw(bytes32)",
// [0x78, 0x42, 0x3b, 0xcf] : "transfer(bytes32,bytes32,uint64)"

#[allow(clippy::too_many_lines)]
pub fn spl_token<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    address: &Address,
    input: &[u8],
    context: &crate::evm::Context,
    is_static: bool,
) -> Result<Vec<u8>> {
    if context.value != 0 {
        return Err(Error::Custom("SplToken: value != 0".to_string()))
    }

    if context.contract == context.caller { 
        return Err(Error::Custom("SplToken: callcode is not allowed".to_string()))
    }

    if (&context.contract != address) && (state.call_depth() != 1) {
        return Err(Error::Custom("SplToken: delegatecall is only allowed in top level contract".to_string()))
    }


    let (selector, input) = input.split_at(4);
    let selector: [u8; 4] = selector.try_into()?;

    match selector {
        [0xb1, 0x1e, 0xcc, 0x50] => { // initializeMint(bytes32 seed, uint8 decimals)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let seed = read_salt(input);
            let decimals = read_u8(&input[32..]);

            initialize_mint(context, state, seed, decimals, None, None)
        }
        [0xc3, 0xf3, 0xf2, 0xf2] => { // initializeMint(bytes32 seed, uint8 decimals, bytes32 mint_authority, bytes32 freeze_authority)
            if is_static { return Err(Error::StaticModeViolation(*address)); }
            
            let seed = read_salt(input);
            let decimals = read_u8(&input[32..]);
            let mint_authority = read_pubkey(&input[64..]);
            let freeze_authority = read_pubkey(&input[96..]);
            initialize_mint(context, state, seed, decimals, Some(mint_authority), Some(freeze_authority))
        }
        [0xda, 0xa1, 0x2c, 0x5c] => { // initializeAccount(bytes32 seed, bytes32 mint)
            if is_static { return Err(Error::StaticModeViolation(*address)); }
            
            let seed = read_salt(input);
            let mint = read_pubkey(&input[32..]);

            initialize_account(context, state, seed, mint, None)
        }
        [0xfc, 0x86, 0xb7, 0x17] => { // initializeAccount(bytes32 seed, bytes32 mint, bytes32 owner)
            if is_static { return Err(Error::StaticModeViolation(*address)); }
            
            let seed = read_salt(input);
            let mint = read_pubkey(&input[32..]);
            let owner = read_pubkey(&input[64..]);
            initialize_account(context, state, seed, mint, Some(owner))
        }
        [0x57, 0x82, 0xa0, 0x43] => { // closeAccount(bytes32 account)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let account = read_pubkey(input);
            close_account(context, state, account)
        }
        [0xa9, 0xc1, 0x58, 0x06] => { // approve(bytes32 source, bytes32 target, uint64 amount)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let source = read_pubkey(input);
            let target = read_pubkey(&input[32..]);
            let amount = read_u64(&input[64..]);
            approve(context, state, source, target, amount)
        }
        [0xb7, 0x5c, 0x7d, 0xc6] => { // revoke(bytes32 source)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let source = read_pubkey(input);
            revoke(context, state, source)
        }
        [0x78, 0x42, 0x3b, 0xcf] => { // transfer(bytes32 source, bytes32 target, uint64 amount)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let source = read_pubkey(input);
            let target = read_pubkey(&input[32..]);
            let amount = read_u64(&input[64..]);
            transfer(context, state, source, target, amount)
        }
        [0xa9, 0x05, 0x74, 0x01] => { // mintTo(bytes32 account, uint64 amount)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let account = read_pubkey(input);
            let amount = read_u64(&input[32..]);
            mint(context, state, account, amount)
        }
        [0xe3, 0x41, 0x08, 0x55] => { // burn(bytes32 account, uint64 amount)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let account = read_pubkey(input);
            let amount = read_u64(&input[32..]);
            burn(context, state, account, amount)
        }
        [0xec, 0x13, 0xcc, 0x7b] => { // freeze(bytes32 account)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let account = read_pubkey(input);
            freeze(context, state, account)
        }
        [0xc2, 0x59, 0xdd, 0xfe] => { // thaw(bytes32 account)
            if is_static { return Err(Error::StaticModeViolation(*address)); }

            let account = read_pubkey(input);
            thaw(context, state, account)
        }
        [0xeb, 0x7d, 0xa7, 0x8c] => { // findAccount(bytes32 seed)
            let seed = read_salt(input);
            find_account(context, state, seed)
        }
        [0x38, 0xa6, 0x99, 0xa4] => { // exists(bytes32 account)
            let account = read_pubkey(input);
            exists(context, state, account)
        }
        [0xd1, 0xde, 0x50, 0x11] => { // getAccount(bytes32 account)
            let account = read_pubkey(input);
            get_account(context, state, account)
        }
        [0xa2, 0xce, 0x9c, 0x1f] => { // getMint(bytes32 account)
            let account = read_pubkey(input);
            get_mint(context, state, account)
        }
        _ => {
            Err(Error::UnknownPrecompileMethodSelector(*address, selector))
        }
    }
}


#[inline]
fn read_u8(input: &[u8]) -> u8 {
    U256::from_be_bytes(*arrayref::array_ref![input, 0, 32]).as_u8()
}

#[inline]
fn read_u64(input: &[u8]) -> u64 {
    U256::from_be_bytes(*arrayref::array_ref![input, 0, 32]).as_u64()
}

#[inline]
fn read_pubkey(input: &[u8]) -> Pubkey {
    Pubkey::new_from_array(*arrayref::array_ref![input, 0, 32])
}

#[inline]
fn read_salt(input: &[u8]) -> &[u8; 32] {
    arrayref::array_ref![input, 0, 32]
}


fn create_account<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    account: &OwnedAccountInfo,
    space: usize,
    seeds: Vec<Vec<u8>>
) -> Result<()> {
    let rent = Rent::get()?;
    let minimum_balance = rent.minimum_balance(space);

    if account.lamports > 0 {
        let required_lamports = minimum_balance.saturating_sub(account.lamports);
        
        if required_lamports > 0 {
            let transfer = system_instruction::transfer(state.backend.operator(), &account.key, required_lamports);
            state.queue_external_instruction(transfer, vec![], 0);
        }

        let allocate = system_instruction::allocate(&account.key, space.try_into().unwrap());
        state.queue_external_instruction(allocate, seeds.clone(), space);

        let assign = system_instruction::assign(&account.key, &spl_token::ID);
        state.queue_external_instruction(assign, seeds, 0);
    } else {
        let create_account = system_instruction::create_account(
            state.backend.operator(),
            &account.key,
            minimum_balance,
            space.try_into().unwrap(),
            &spl_token::ID,
        );
        state.queue_external_instruction(create_account, seeds, space);
    }

    Ok(())
}

fn initialize_mint<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    seed: &[u8],
    decimals: u8,
    mint_authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
) -> Result<Vec<u8>> {
    let signer = context.caller;
    let (signer_pubkey, _) = state.backend.solana_address(&signer);

    let (mint_key, bump_seed) = Pubkey::find_program_address(
        &[ &[ACCOUNT_SEED_VERSION], b"ContractData", signer.as_bytes(), seed ], 
        state.backend.program_id()
    );

    let account = state.external_account(mint_key)?;
    if !system_program::check_id(&account.owner) {
        return Err(Error::AccountInvalidOwner(mint_key, account.owner, system_program::ID));
    }

    let seeds: Vec<Vec<u8>> = vec![
        vec![ACCOUNT_SEED_VERSION], b"ContractData".to_vec(),
        signer.as_bytes().to_vec(), seed.to_vec(),
        vec![bump_seed]
    ];

    create_account(state, &account, spl_token::state::Mint::LEN, seeds)?;

    let initialize_mint = spl_token::instruction::initialize_mint(
        &spl_token::ID,
        &mint_key,
        &mint_authority.unwrap_or(signer_pubkey),
        Some(&freeze_authority.unwrap_or(signer_pubkey)),
        decimals
    )?;
    state.queue_external_instruction(initialize_mint, vec![], 0);

    Ok(mint_key.to_bytes().to_vec())
}

fn initialize_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    seed: &[u8],
    mint: Pubkey,
    owner: Option<Pubkey>,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, _) = state.backend.solana_address(&signer);

    let (account_key, bump_seed) = Pubkey::find_program_address(
        &[ &[ACCOUNT_SEED_VERSION], b"ContractData", signer.as_bytes(), seed ], 
        state.backend.program_id()
    );

    let account = state.external_account(account_key)?;
    if !system_program::check_id(&account.owner) {
        return Err(Error::AccountInvalidOwner(account_key, account.owner, system_program::ID));
    }

    let seeds: Vec<Vec<u8>> = vec![
        vec![ACCOUNT_SEED_VERSION], b"ContractData".to_vec(),
        signer.as_bytes().to_vec(), seed.to_vec(),
        vec![bump_seed]
    ];

    create_account(state, &account, spl_token::state::Account::LEN, seeds)?;

    let initialize_mint = spl_token::instruction::initialize_account2(
        &spl_token::ID,
        &account_key,
        &mint,
        &owner.unwrap_or(signer_pubkey)
    )?;
    state.queue_external_instruction(initialize_mint, vec![], 0);

    Ok(account_key.to_bytes().to_vec())
}

fn close_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let close_account = spl_token::instruction::close_account(
        &spl_token::ID,
        &account,
        state.backend.operator(),
        &signer_pubkey,
        &[]
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
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let approve = spl_token::instruction::approve(
        &spl_token::ID,
        &source,
        &target,
        &signer_pubkey,
        &[],
        amount
    )?;
    state.queue_external_instruction(approve, seeds, 0);

    Ok(vec![])
}

fn revoke<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let revoke = spl_token::instruction::revoke(
        &spl_token::ID,
        &account,
        &signer_pubkey,
        &[]
    )?;
    state.queue_external_instruction(revoke, seeds, 0);

    Ok(vec![])
}

fn transfer<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    source: Pubkey,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let transfer = spl_token::instruction::transfer(
        &spl_token::ID,
        &source,
        &target,
        &signer_pubkey,
        &[],
        amount
    )?;
    state.queue_external_instruction(transfer, seeds, 0);

    Ok(vec![])
}

fn mint<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    target: Pubkey,
    amount: u64,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);

    let account = state.external_account(target)?;
    spl_token::check_program_account(&account.owner)?;

    let token_account = spl_token::state::Account::unpack(&account.data)?;

    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let mint_to = spl_token::instruction::mint_to(
        &spl_token::ID,
        &token_account.mint,
        &target,
        &signer_pubkey,
        &[],
        amount
    )?;
    state.queue_external_instruction(mint_to, seeds, 0);

    Ok(vec![])
}

fn burn<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    source: Pubkey,
    amount: u64,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);
    
    let account = state.external_account(source)?;
    spl_token::check_program_account(&account.owner)?;
    
    let token_account = spl_token::state::Account::unpack(&account.data)?;
    
    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let burn = spl_token::instruction::burn(
        &spl_token::ID,
        &source,
        &token_account.mint,
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
    target: Pubkey,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);
    
    let account = state.external_account(target)?;
    spl_token::check_program_account(&account.owner)?;
    
    let token_account = spl_token::state::Account::unpack(&account.data)?;
    
    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let freeze = spl_token::instruction::freeze_account(
        &spl_token::ID,
        &target,
        &token_account.mint,
        &signer_pubkey,
        &[],
    )?;
    state.queue_external_instruction(freeze, seeds, 0);

    Ok(vec![])
}

fn thaw<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    target: Pubkey,
) -> Result<Vec<u8>>
{
    let signer = context.caller;
    let (signer_pubkey, bump_seed) = state.backend.solana_address(&signer);
    
    let account = state.external_account(target)?;
    spl_token::check_program_account(&account.owner)?;
    
    let token_account = spl_token::state::Account::unpack(&account.data)?;
    
    let seeds = vec![ vec![ACCOUNT_SEED_VERSION], signer.as_bytes().to_vec(), vec![bump_seed] ];

    let thaw = spl_token::instruction::thaw_account(
        &spl_token::ID,
        &target,
        &token_account.mint,
        &signer_pubkey,
        &[],
    )?;
    state.queue_external_instruction(thaw, seeds, 0);

    Ok(vec![])
}

#[allow(clippy::unnecessary_wraps)]
fn find_account<B: AccountStorage>(
    context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    seed: &[u8]
) -> Result<Vec<u8>>
{
    let signer = context.caller;

    let (account_key, _) = Pubkey::find_program_address(
        &[ &[ACCOUNT_SEED_VERSION], b"ContractData", signer.as_bytes(), seed ], 
        state.backend.program_id()
    );

    Ok(account_key.to_bytes().to_vec())
}

fn exists<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>>
{
    let account = state.external_account(account)?;
    if system_program::check_id(&account.owner) {
        Ok(vec![0_u8; 32])
    } else {
        let mut result = vec![0_u8; 32];
        result[31] = 1; // return true

        Ok(result)
    }
}

fn get_account<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>>
{
    let account = state.external_account(account)?;
    let token = if spl_token::check_id(&account.owner) {
        spl_token::state::Account::unpack_unchecked(&account.data)?
    } else {
        spl_token::state::Account::default()
    };

    debug_print!("spl_token get_account: {:?}", token);

    let mut result = [0_u8; 7*32];
    let (mint, owner, _, amount, delegate, _, delegated_amount, close_authority, state) 
        = arrayref::mut_array_refs![&mut result, 32, 32, 24, 8, 32, 24, 8, 32, 32];

    *mint = token.mint.to_bytes();
    *owner = token.owner.to_bytes();
    *amount = token.amount.to_be_bytes();
    *delegate = token.delegate.map(Pubkey::to_bytes).unwrap_or_default();
    *delegated_amount = token.delegated_amount.to_be_bytes();
    *close_authority = token.close_authority.map(Pubkey::to_bytes).unwrap_or_default();
    state[31] = token.state as u8;

    Ok(result.to_vec())
}

fn get_mint<B: AccountStorage>(
    _context: &crate::evm::Context,
    state: &mut ExecutorState<B>,
    account: Pubkey,
) -> Result<Vec<u8>>
{
    let account = state.external_account(account)?;
    let mint = if spl_token::check_id(&account.owner) {
        spl_token::state::Mint::unpack_unchecked(&account.data)?
    } else {
        spl_token::state::Mint::default()
    };

    debug_print!("spl_token get_mint: {:?}", mint);

    let mut result = [0_u8; 5*32];
    let (_, supply, _, decimals, _, is_initialized, freeze_authority, mint_authority ) 
        = arrayref::mut_array_refs![&mut result, 24, 8, 31, 1, 31, 1, 32, 32];

    *supply = mint.supply.to_be_bytes();
    *decimals = mint.decimals.to_be_bytes();
    *is_initialized = if mint.is_initialized { [1_u8] } else { [0_u8] };
    *freeze_authority = mint.freeze_authority.map(Pubkey::to_bytes).unwrap_or_default();
    *mint_authority = mint.mint_authority.map(Pubkey::to_bytes).unwrap_or_default();

    Ok(result.to_vec())
}