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

const COLLATERAL_POOL_BASE: &str = "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT";
const COLLATERAL_SEED_PREFIX: &str = "collateral_seed_";
const PAYMENT_TO_COLLATERAL_POOL: u64 = 1000;

/// Checks collateral accounts for the Ethereum transaction execution.
#[allow(clippy::unnecessary_wraps)]
#[allow(unused_variables)]
pub fn check_collateral_account(program_id: &Pubkey,
                                collateral_pool_sol_info: &AccountInfo,
                                collateral_pool_index: usize) -> ProgramResult {
    debug_print!("program_id {}", program_id);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);
    debug_print!("collateral_pool_index {}", collateral_pool_index);

    // Owner of collateral_pool_sol_info is system: 11111111111111111111111111111111
    /*if collateral_pool_sol_info.owner != program_id {
        debug_print!("Wrong collateral pool owner {}", *collateral_pool_sol_info.owner);
        debug_print!("Must be program_id {}", program_id);
        return Err(ProgramError::InvalidArgument);
    }*/

    let collateral_pool_key = Pubkey::from_str(COLLATERAL_POOL_BASE)
        .map_err(|e| {
            debug_print!("Error key string '{}', {:?}", COLLATERAL_POOL_BASE, e);
            ProgramError::InvalidArgument
        })?;

    let seed = format!("{}{}", COLLATERAL_SEED_PREFIX, collateral_pool_index);
    let pool_key = Pubkey::create_with_seed(&collateral_pool_key, &seed, program_id)?;
    if *collateral_pool_sol_info.key != pool_key {
        debug_print!("Wrong seed pool key {}", pool_key);
        debug_print!("Must be collateral pool key {}", *collateral_pool_sol_info.key);
        return Err(ProgramError::InvalidArgument);
    }

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
#[allow(clippy::unnecessary_wraps)]
pub fn from_operator_to_collateral_pool<'a>(operator_sol_info: &'a AccountInfo<'a>,
                    collateral_pool_sol_info: &'a AccountInfo<'a>,
                    system_info: &'a AccountInfo<'a>) -> ProgramResult {
    debug_print!("operator_sol_info {:?}", operator_sol_info);
    let transfer = system_instruction::transfer(operator_sol_info.key,
                                                collateral_pool_sol_info.key,
                                                PAYMENT_TO_COLLATERAL_POOL);
    let accounts = [(*operator_sol_info).clone(),
        (*collateral_pool_sol_info).clone(),
        (*system_info).clone()];
    invoke(&transfer, &accounts)?;

    Ok(())
}
