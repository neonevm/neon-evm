//! `evm_loader` program payment module.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

// use std::str::FromStr;

// TODO set collateral pool base address
// const COLLATERAL_POOL_BASE: &str = "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT";

/// `COLLATERAL_SEED_PREFIX`
pub const COLLATERAL_SEED_PREFIX: &str = "collateral_seed_";
/// `PAYMENT_TO_COLLATERAL_POOL`
pub const PAYMENT_TO_COLLATERAL_POOL: u64 = 1000;
/// `PAYMENT_TO_DEPOSIT`
pub const PAYMENT_TO_DEPOSIT: u64 = 1000;

/// Checks collateral accounts for the Ethereum transaction execution.
/// # Errors
///
/// Will return: 
/// `ProgramError::InvalidArgument` if `collateral_pool_sol_info` owner not `program_id` 
/// or its key is not equal to generated
pub fn check_collateral_account(
    program_id: &Pubkey,
    // WARNING Only for tests when base is random
    collateral_pool_base: &AccountInfo,
    collateral_pool_sol_info: &AccountInfo,
    collateral_pool_index: usize
) -> ProgramResult {
    debug_print!("program_id {}", program_id);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);
    debug_print!("collateral_pool_index {}", collateral_pool_index);

    // Owner of collateral_pool_sol_info is system: 11111111111111111111111111111111
    /*if collateral_pool_sol_info.owner != program_id {
        debug_print!("Wrong collateral pool owner {}", *collateral_pool_sol_info.owner);
        debug_print!("Must be program_id {}", program_id);
        return Err(ProgramError::InvalidArgument);
    }*/

    let collateral_pool_key = collateral_pool_base.key;

    let seed = format!("{}{}", COLLATERAL_SEED_PREFIX, collateral_pool_index);
    let pool_key = Pubkey::create_with_seed(collateral_pool_key, &seed, program_id)?;
    if *collateral_pool_sol_info.key != pool_key {
        debug_print!("Wrong seed pool key {}", pool_key);
        debug_print!("Must be collateral pool key {}", *collateral_pool_sol_info.key);
        return Err(ProgramError::InvalidArgument);
    }

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error only if `transfer` fail
pub fn transfer_from_operator_to_collateral_pool<'a>(
    operator_sol_info: &'a AccountInfo<'a>,
    collateral_pool_sol_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>
) -> ProgramResult {
    debug_print!("operator_to_collateral_pool");
    debug_print!("operator_sol_info {:?}", operator_sol_info);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);

    transfer(operator_sol_info, collateral_pool_sol_info, system_info, PAYMENT_TO_COLLATERAL_POOL)?;

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error if `transfer` fail
/// or
/// `ProgramError::InsufficientFunds` if deposit account have not enough funds for year rent
pub fn transfer_from_operator_to_deposit<'a>(
    operator_sol_info: &'a AccountInfo<'a>,
    deposit_sol_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>
) -> ProgramResult {
    debug_print!("operator_to_deposit");
    debug_print!("operator_sol_info {:?}", operator_sol_info);
    debug_print!("deposit_sol_info {:?}", deposit_sol_info);

    let rent_via_sysvar = Rent::get()?;
    if rent_via_sysvar.lamports_per_byte_year * deposit_sol_info.data.borrow().len() as u64 > deposit_sol_info.lamports() {
        debug_print!("deposit account insufficient funds");
        debug_print!("lamports_per_byte_year {}", rent_via_sysvar.lamports_per_byte_year);
        debug_print!("deposit_sol_info.data.len() {}",  deposit_sol_info.data.borrow().len());
        debug_print!("deposit_sol_info.lamports() {}", deposit_sol_info.lamports());
        return Err(ProgramError::InsufficientFunds)
    }

    transfer(operator_sol_info, deposit_sol_info, system_info, PAYMENT_TO_DEPOSIT)?;

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error only if `transfer` fail
pub fn transfer_from_deposit_to_operator<'a>(
    deposit_sol_info: &'a AccountInfo<'a>,
    operator_sol_info: &'a AccountInfo<'a>,
    _system_info: &'a AccountInfo<'a>
) -> ProgramResult {
    debug_print!("deposit_to_operator");
    debug_print!("deposit_sol_info {:?}", deposit_sol_info);
    debug_print!("operator_sol_info {:?}", operator_sol_info);

    if deposit_sol_info.lamports() < PAYMENT_TO_DEPOSIT {
        return Err(ProgramError::InsufficientFunds)
    }

    **deposit_sol_info.lamports.borrow_mut() = deposit_sol_info.lamports() - PAYMENT_TO_DEPOSIT;
    **operator_sol_info.lamports.borrow_mut() = operator_sol_info.lamports() + PAYMENT_TO_DEPOSIT;

    Ok(())
}

fn transfer<'a>(
    from_account_info: &'a AccountInfo<'a>,
    to_account_info: &'a AccountInfo<'a>,
    system_info: &'a AccountInfo<'a>,
    amount: u64
) -> ProgramResult {
    let transfer = system_instruction::transfer(from_account_info.key,
                                                to_account_info.key,
                                                amount);
    let accounts = [(*from_account_info).clone(),
        (*to_account_info).clone(),
        (*system_info).clone()];
    invoke(&transfer, &accounts)?;

    Ok(())
}