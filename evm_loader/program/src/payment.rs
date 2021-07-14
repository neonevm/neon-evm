//! `evm_loader` program payment module.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};

use std::str::FromStr;

const COLLATERAL_POOL: &str = "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT";
const COLLATERAL_SEEDS: &[&str] = &[
    "collateral_seed_0",
    "collateral_seed_1",
    "collateral_seed_2",
    "collateral_seed_3",
    "collateral_seed_4",
];

const PAYMENT_TO_COLLATERAL_POOL: u64 = 1000;

/// Checks collateral accounts for the Ethereum transaction execution.
#[allow(clippy::unnecessary_wraps)]
pub fn check_collateral_account(program_id: &Pubkey,
                                collateral_pool_sol_info: &AccountInfo,
                                collateral_pool_seed_index: usize) -> ProgramResult {
    debug_print!("program_id {:?}", program_id);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);
    debug_print!("collateral_pool_seed_index {}", collateral_pool_seed_index);

    let collateral_pool = Pubkey::from_str(COLLATERAL_POOL)
        .map_err(|e| {
            debug_print!("Error key string '{}', {:?}", COLLATERAL_POOL, e);
            ProgramError::InvalidArgument
        })?;

    if collateral_pool_seed_index >= COLLATERAL_SEEDS.len() {
        debug_print!("Error: seed index {} out of range [0..{}]",
            collateral_pool_seed_index,
            COLLATERAL_SEEDS.len() - 1);
        return Err(ProgramError::InvalidInstructionData);
    }

    let seed = COLLATERAL_SEEDS[collateral_pool_seed_index];
    let pool_key = Pubkey::create_with_seed(&collateral_pool, seed, program_id)?;
    if *collateral_pool_sol_info.key != pool_key {
        debug_print!("Collateral pool key {}", *collateral_pool_sol_info.key);
        debug_print!("Wrong seed pool key {}", pool_key);
        return Err(ProgramError::InvalidArgument);
    }

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
#[allow(clippy::unnecessary_wraps)]
pub fn operator<'a>(operator_sol_info: &'a AccountInfo<'a>,
                    collateral_pool_sol_info: &'a AccountInfo<'a>,
                    system_info: &'a AccountInfo<'a>) -> ProgramResult {
    let transfer = system_instruction::transfer(operator_sol_info.key,
                                                collateral_pool_sol_info.key,
                                                PAYMENT_TO_COLLATERAL_POOL);
    let accounts = [(*operator_sol_info).clone(),
        (*collateral_pool_sol_info).clone(),
        (*system_info).clone()];
    invoke(&transfer, &accounts)?;

    Ok(())
}
