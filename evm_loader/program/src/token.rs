//! `EVMLoader` token functions
use crate::{
    account_data::{AccountData, ACCOUNT_SEED_VERSION},
    solidity_account::SolidityAccount,
    neon::token_mint,
    storage_account::StorageAccount,
    account_storage::ProgramAccountStorage,
};
use evm::{U256};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    account_info::{AccountInfo},
    pubkey::Pubkey,
    system_program, sysvar,
    program_error::ProgramError,
    program_pack::Pack,
    program::invoke_signed,
    msg,
};
use std::vec;
use std::convert::TryFrom;

#[must_use]
/// Number of base 10 digits to the right of the decimal place of ETH value
pub const fn eth_decimals() -> u8 { 18 }

/// Create an associated token account for the given wallet address and token mint
#[must_use]
pub fn create_associated_token_account(
    funding_address: &Pubkey,
    wallet_address: &Pubkey,
    token_account_address: &Pubkey,
    spl_token_mint_address: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: spl_associated_token_account::id(),
        accounts: vec![
            AccountMeta::new(*funding_address, true),
            AccountMeta::new(*token_account_address, false),
            AccountMeta::new_readonly(*wallet_address, false),
            AccountMeta::new_readonly(*spl_token_mint_address, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data: vec![],
    }
}


/// Extract a token amount from the `AccountInfo`
/// 
/// # Errors
///
/// Will return: 
/// `ProgramError::IncorrectProgramId` if account is not token account
pub fn get_token_account_balance(account: &AccountInfo) -> Result<u64, ProgramError> {
    if account.data_len() == 0 {
        return Ok(0_u64);
    }

    if *account.owner != spl_token::id() {
        return Err!(ProgramError::IncorrectProgramId; "*account.owner<{:?}> != spl_token::id()<{:?}>", *account.owner,  spl_token::id());
    }

    let data = spl_token::state::Account::unpack(&account.data.borrow())?;

    Ok(data.amount)
}

/// Extract a token owner from `AccountInfo`
/// 
/// # Errors
///
/// Will return: 
/// `ProgramError::IncorrectProgramId` if account is not token account
pub fn get_token_account_owner(account: &AccountInfo) -> Result<Pubkey, ProgramError> {
    if *account.owner != spl_token::id() {
        return Err!(ProgramError::IncorrectProgramId; "Invalid account owner");
    }

    let data = spl_token::state::Account::unpack(&account.data.borrow())?;

    Ok(data.owner)
}

/// Extract a token mint data from the account data
///
/// # Errors
///
/// Will return:
/// `ProgramError::IncorrectProgramId` if account is not token mint account
pub fn get_token_mint_data(data: &[u8], owner: &Pubkey) -> Result<spl_token::state::Mint, ProgramError> {
    if *owner != spl_token::id() {
        return Err!(ProgramError::IncorrectProgramId; "*owner<{:?}> != spl_token::id()<{:?}>", *owner,  spl_token::id());
    }

    spl_token::state::Mint::unpack(data)
}

/// Extract a token account data from the account data
///
/// # Errors
///
/// Will return:
/// `ProgramError::IncorrectProgramId` if account is not token mint account
pub fn get_token_account_data(data: &[u8], owner: &Pubkey) -> Result<spl_token::state::Account, ProgramError> {
    if *owner != spl_token::id() {
        return Err!(ProgramError::IncorrectProgramId; "*owner<{:?}> != spl_token::id()<{:?}>", *owner,  spl_token::id());
    }

    spl_token::state::Account::unpack(data)
}


/// Validate Token Account
/// 
/// # Errors
///
/// Will return: 
/// `ProgramError::IncorrectProgramId` if account is not token account
pub fn check_token_account(token: &AccountInfo, account: &AccountInfo) -> Result<(), ProgramError> {
    debug_print!("check_token_account");
    if *token.owner != spl_token::id() {
        return Err!(ProgramError::IncorrectProgramId; "token.owner != spl_token::id() {}", token.owner);
    }

    let data = account.try_borrow_data()?;
    let data = AccountData::unpack(&data)?;
    let data = data.get_account()?;
    if data.eth_token_account != *token.key {
        return Err!(ProgramError::IncorrectProgramId; "data.eth_token_account != *token.key data.eth = {} token.key = {}", data.eth_token_account, *token.key);
    }

    debug_print!("check_token_account success");

    Ok(())
}


/// Transfer Tokens
/// 
/// # Errors
///
/// Could return: 
/// `ProgramError::InvalidInstructionData`
pub fn transfer_token(
    accounts: &[AccountInfo],
    source_token_account: &AccountInfo,
    target_token_account: &AccountInfo,
    source_account: &AccountInfo,
    source_solidity_account: &SolidityAccount,
    value: &U256,
) -> Result<(), ProgramError> {
    debug_print!("transfer_token");
    if get_token_account_owner(source_token_account)? != *source_account.key {
        return Err!(ProgramError::InvalidInstructionData;
            "Invalid account owner;\
             source_token_account = {:?},\
              source_account = {:?}",
            source_token_account,
            source_account
        );
    }

    let min_decimals = u32::from(eth_decimals() - token_mint::decimals());
    let min_value = U256::from(10_u64.pow(min_decimals));
    let value = value / min_value;
    let value = u64::try_from(value).map_err(|_| E!(ProgramError::InvalidInstructionData))?;

    let source_token_balance = get_token_account_balance(source_token_account)?;
    if source_token_balance < value {
        return Err!(ProgramError::InvalidInstructionData;
            "Insufficient funds on token account {:?} {:?}",
            source_token_account,
            source_token_balance
        )
    }

    msg!("Transfer NEON tokens from {} to {} value {}", source_token_account.key, target_token_account.key, value);

    let instruction = spl_token::instruction::transfer_checked(
        &spl_token::id(),
        source_token_account.key,
        &token_mint::id(),
        target_token_account.key,
        source_account.key,
        &[],
        value,
        token_mint::decimals(),
    )?;

    let (ether, nonce) = source_solidity_account.get_seeds();
    let program_seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], ether.as_bytes(), &[nonce]];
    invoke_signed(&instruction, accounts, &[program_seeds])?;

    Ok(())
}


/// A neon-evm user pays an operator
///
/// # Errors
///
/// Could return:
/// `ProgramError::InvalidArgument`
#[allow(clippy::too_many_arguments)]
pub fn user_pays_operator<'a>(
    gas_limit: u64,
    gas_price: u64,
    used_gas: u64,
    user_token_account: &'a AccountInfo<'a>,
    operator_token_account: &'a AccountInfo<'a>,
    accounts: &'a [AccountInfo<'a>],
    account_storage: &ProgramAccountStorage,
    storage_opt: Option<&mut StorageAccount>,
) -> Result<(), ProgramError> {
    if let Some(storage) = storage_opt {
        let (gas_used_and_paid, number_of_payments) =
            storage.get_payments_info()?;

        let gas_price_wei = U256::from(gas_price);
        msg!("user_pays_operator gas_used_and_paid ={:?}; used_gas={:?} by an iteration N = {:?}",
            gas_used_and_paid, used_gas, number_of_payments+1);

        if used_gas > gas_limit {
            return Err!(ProgramError::InvalidArgument;
                "used_gas > gas_limit; gas_to_be_paid={:?}; gas_limit={:?}",
                used_gas, gas_limit);
        }

        let gas_to_be_paid = used_gas.checked_sub(gas_used_and_paid)
            .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

        let fee = U256::from(gas_to_be_paid)
            .checked_mul(gas_price_wei)
            .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

        transfer_token(
            accounts,
            user_token_account,
            operator_token_account,
            account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?,
            account_storage.get_caller_account().ok_or_else(|| E!(ProgramError::InvalidArgument))?,
            &fee)?;

        let gas_has_been_paid = gas_to_be_paid;
        msg!("user_pays_operator gas_has_been_paid ={:?}", gas_has_been_paid);
        storage.set_gas_has_been_paid(gas_has_been_paid)?;
    }
    else {
        let (gas_used_and_paid, _number_of_payments) = (0, 0);

        let gas_to_be_paid = used_gas.checked_sub(gas_used_and_paid)
            .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

        let gas_price_wei = U256::from(gas_price);

        let fee = U256::from(gas_to_be_paid)
            .checked_mul(gas_price_wei)
            .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

        transfer_token(
            accounts,
            user_token_account,
            operator_token_account,
            account_storage.get_caller_account_info().ok_or_else(||E!(ProgramError::InvalidArgument))?,
            account_storage.get_caller_account().ok_or_else(|| E!(ProgramError::InvalidArgument))?,
            &fee)?;

        let gas_has_been_paid = gas_to_be_paid;
        msg!("user_pays_operator gas_has_been_paid ={:?}", gas_has_been_paid);
    }

    Ok(())
}
