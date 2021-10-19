//! `evm_loader` program payment module.

use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    entrypoint::ProgramResult,
    incinerator,
    program::invoke,
    program_error::ProgramError,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

use crate::neon::collateral_pool_base;


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
fn check_collateral_account(
    program_id: &Pubkey,
    collateral_pool_sol_info: &AccountInfo,
    collateral_pool_index: u32
) -> ProgramResult {
    debug_print!("program_id {}", program_id);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);
    debug_print!("collateral_pool_index {}", collateral_pool_index);

    if collateral_pool_sol_info.owner != program_id {
        debug_print!("Wrong collateral pool owner {}", *collateral_pool_sol_info.owner);
        debug_print!("Must be program_id {}", program_id);
        return Err!(ProgramError::InvalidArgument; "Wrong collateral pool owner");
    }

    let seed = format!("{}{}", collateral_pool_base::PREFIX, collateral_pool_index);
    let pool_key = Pubkey::create_with_seed(&collateral_pool_base::id(), &seed, program_id)?;
    if *collateral_pool_sol_info.key != pool_key {
        debug_print!("Wrong seed pool key {}", pool_key);
        debug_print!("Must be collateral pool key {}", *collateral_pool_sol_info.key);
        return Err!(ProgramError::InvalidArgument; "Wrong seed for collateral pool key");
    }

    Ok(())
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error only if `transfer` fail
pub fn transfer_from_operator_to_collateral_pool<'a>(
    program_id: &Pubkey,
    collateral_pool_index: u32,
    operator_sol_info: &AccountInfo<'a>,
    collateral_pool_sol_info: &AccountInfo<'a>,
    system_info: &AccountInfo<'a>
) -> ProgramResult {
    check_collateral_account(
        program_id,
        collateral_pool_sol_info,
        collateral_pool_index)?;
    debug_print!("operator_to_collateral_pool");
    debug_print!("operator_sol_info {:?}", operator_sol_info);
    debug_print!("collateral_pool_sol_info {:?}", collateral_pool_sol_info);

    transfer_system_owned(operator_sol_info, collateral_pool_sol_info, system_info, PAYMENT_TO_COLLATERAL_POOL)
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error if `transfer` fail
/// or
/// `ProgramError::InsufficientFunds` if deposit account have not enough funds for year rent
pub fn transfer_from_operator_to_deposit<'a>(
    operator_sol_info: &AccountInfo<'a>,
    deposit_sol_info: &AccountInfo<'a>,
    system_info: &AccountInfo<'a>
) -> ProgramResult {
    debug_print!("operator_to_deposit");
    debug_print!("operator_sol_info {:?}", operator_sol_info);
    debug_print!("deposit_sol_info {:?}", deposit_sol_info);

    let rent_via_sysvar = Rent::get()?;
    let rent_exempt_balance = rent_via_sysvar.minimum_balance(deposit_sol_info.data_len());
    if rent_exempt_balance > deposit_sol_info.lamports() {
        debug_print!("deposit account insufficient funds");
        debug_print!("rent_exempt_balance {}", rent_exempt_balance);
        debug_print!("deposit_sol_info.data.len() {}", deposit_sol_info.data_len());
        debug_print!("deposit_sol_info.lamports() {}", deposit_sol_info.lamports());

        let funds_to_rent_exempt = rent_exempt_balance - deposit_sol_info.lamports();
        debug_print!("add funds to rents exempt");
        transfer_system_owned(operator_sol_info, deposit_sol_info, system_info, funds_to_rent_exempt)?;
    }

    transfer_system_owned(operator_sol_info, deposit_sol_info, system_info, PAYMENT_TO_DEPOSIT)
}

/// Makes payments for the Ethereum transaction execution.
/// # Errors
///
/// Will return error only if `transfer` fail
pub fn transfer_from_deposit_to_operator(
    deposit_sol_info: &AccountInfo,
    operator_sol_info: &AccountInfo,
) -> ProgramResult {
    debug_print!("deposit_to_operator");
    debug_print!("deposit_sol_info {:?}", deposit_sol_info);
    debug_print!("operator_sol_info {:?}", operator_sol_info);

    transfer_program_owned(deposit_sol_info, operator_sol_info, PAYMENT_TO_DEPOSIT)
}


/// Burns deposit
/// # Errors
///
/// Will return error only if `transfer` fail
pub fn burn_operators_deposit(
    deposit_sol_info: &AccountInfo,
    incinerator_info: &AccountInfo,
) -> ProgramResult {
    if !incinerator::check_id(incinerator_info.key) {
        return Err!(ProgramError::InvalidAccountData; "Must be incinerator key")
    }

    debug_print!("deposit_to_operator");
    debug_print!("deposit_sol_info {:?}", deposit_sol_info);
    debug_print!("incinerator {:?}", incinerator_info);

    transfer_program_owned(deposit_sol_info, incinerator_info, PAYMENT_TO_DEPOSIT)
}


fn transfer_system_owned<'a>(
    from_account_info: &AccountInfo<'a>,
    to_account_info: &AccountInfo<'a>,
    system_info: &AccountInfo<'a>,
    amount: u64
) -> ProgramResult {
    let transfer = system_instruction::transfer(
        from_account_info.key,
        to_account_info.key,
        amount
    );
    let accounts = [
        from_account_info.clone(),
        to_account_info.clone(),
        system_info.clone()
    ];

    invoke(&transfer, &accounts)
}

fn transfer_program_owned(
    from_account_info: &AccountInfo,
    to_account_info: &AccountInfo,
    amount: u64
) -> ProgramResult {
    if from_account_info.lamports() < amount {
        return Err!(ProgramError::InsufficientFunds)
    }

    **from_account_info.lamports.borrow_mut() -= amount;
    **to_account_info.lamports.borrow_mut() += amount;

    Ok(())
}