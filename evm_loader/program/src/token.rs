//! `EVMLoader` token functions
use crate::{
    account_data::{AccountData},
    solidity_account::SolidityAccount
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
};
use std::vec;
use std::convert::TryFrom;

/// Token Mint ID
pub mod token_mint {
    solana_program::declare_id!("HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");

    /// Number of base 10 digits to the right of the decimal place
    #[must_use]
    pub const fn decimals() -> u8 { 9 }
}

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
        debug_print!("source ownership");
        debug_print!("source owner {}", get_token_account_owner(source_token_account)?);
        debug_print!("source key {}", source_account.key);
        return Err!(ProgramError::InvalidInstructionData; "Invalid account owner")
    }

    let min_decimals = u32::from(eth_decimals() - token_mint::decimals());
    let min_value = U256::from(10_u64.pow(min_decimals));
    let value = value / min_value;
    let value = u64::try_from(value).map_err(|_| E!(ProgramError::InvalidInstructionData))?;

    debug_print!("Transfer ETH tokens from {} to {} value {}", source_token_account.key, target_token_account.key, value);

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
    invoke_signed(&instruction, accounts, &[&[ether.as_bytes(), &[nonce]]])?;

    Ok(())
}


/// Transfer Tokens to block account
/// 
/// # Errors
///
/// Could return: 
/// `ProgramError::InvalidInstructionData`
pub fn block_token(
    accounts: &[AccountInfo],
    source_token_account: &AccountInfo,
    target_token_account: &AccountInfo,
    source_account: &AccountInfo,
    source_solidity_account: &SolidityAccount,
    value: &U256,
) -> Result<(), ProgramError> {
    let (ether, _nonce) = source_solidity_account.get_seeds();
    debug_print!("block_token");
    if *source_token_account.key != spl_associated_token_account::get_associated_token_address(source_account.key, &token_mint::id()) {
        debug_print!("invalid user token account");
        debug_print!("target: {}", source_token_account.key);
        debug_print!("expected: {}", spl_associated_token_account::get_associated_token_address(source_account.key, &token_mint::id()));
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account")
    }
    if get_token_account_owner(target_token_account)? != *source_account.key {
        debug_print!("target ownership");
        debug_print!("target owner {}", get_token_account_owner(target_token_account)?);
        debug_print!("source key {}", source_account.key);
        return Err!(ProgramError::InvalidInstructionData; "Invalid account owner")
    }
    let holder_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "hold";
    if *target_token_account.key != Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())? {
        debug_print!("invalid hold token account");
        debug_print!("target: {}", target_token_account.key);
        debug_print!("expected: {}", Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())?);
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account")
    }

    transfer_token(
        accounts,
        source_token_account,
        target_token_account,
        source_account,
        source_solidity_account,
        value,
    )?;

    Ok(())
}


/// Transfer Tokens from block account to operator
/// 
/// # Errors
///
/// Could return: 
/// `ProgramError::InvalidInstructionData`
pub fn pay_token(
    accounts: &[AccountInfo],
    source_token_account: &AccountInfo,
    target_token_account: &AccountInfo,
    source_account: &AccountInfo,
    source_solidity_account: &SolidityAccount,
    value: &U256,
) -> Result<(), ProgramError> {
    let (ether, _nonce) = source_solidity_account.get_seeds();
    debug_print!("pay_token");
    let holder_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "hold";
    if *source_token_account.key != Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())? {
        debug_print!("invalid hold token account");
        debug_print!("target: {}", source_token_account.key);
        debug_print!("expected: {}", Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())?);
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account")
    }

    transfer_token(
        accounts,
        source_token_account,
        target_token_account,
        source_account,
        source_solidity_account,
        value,
    )?;

    Ok(())
}


/// Return Tokens from block account to user
/// 
/// # Errors
///
/// Could return: 
/// `ProgramError::InvalidInstructionData`
pub fn return_token(
    accounts: &[AccountInfo],
    source_token_account: &AccountInfo,
    target_token_account: &AccountInfo,
    source_account: &AccountInfo,
    source_solidity_account: &SolidityAccount,
    value: &U256,
) -> Result<(), ProgramError> {
    let (ether, _nonce) = source_solidity_account.get_seeds();
    debug_print!("return_token");
    let holder_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "hold";
    if *source_token_account.key != Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())? {
        debug_print!("invalid hold token account");
        debug_print!("target: {}", source_token_account.key);
        debug_print!("expected: {}", Pubkey::create_with_seed(source_account.key, &holder_seed, &spl_token::id())?);
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account")
    }
    if get_token_account_owner(target_token_account)? != *source_account.key {
        debug_print!("target ownership");
        debug_print!("target owner {}", get_token_account_owner(target_token_account)?);
        debug_print!("source key {}", source_account.key);
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account owner")
    }
    if *target_token_account.key != spl_associated_token_account::get_associated_token_address(source_account.key, &token_mint::id()) {
        debug_print!("invalid user token account");
        debug_print!("target: {}", target_token_account.key);
        debug_print!("expected: {}", spl_associated_token_account::get_associated_token_address(source_account.key, &token_mint::id()));
        return Err!(ProgramError::InvalidInstructionData; "Invalid token account")
    }

    transfer_token(
        accounts,
        source_token_account,
        target_token_account,
        source_account,
        source_solidity_account,
        value,
    )?;

    Ok(())
}