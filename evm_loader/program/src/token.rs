//! `EVMLoader` token functions
use crate::{
    account_data::{AccountData, ACCOUNT_SEED_VERSION},
    solidity_account::SolidityAccount,
    storage_account::StorageAccount,
    account_storage::ProgramAccountStorage,
    config::token_mint,
    error::EvmLoaderError,
};
use evm::{U256};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    account_info::{AccountInfo},
    pubkey::Pubkey,
    system_program, sysvar,
    program_error::ProgramError,
    program_pack::Pack,
    program::invoke_signed
};
use std::vec;
use std::convert::TryFrom;

/// Native token info
pub mod eth {
    use super::U256;

    #[must_use]
    /// Number of base 10 digits to the right of the decimal place of ETH value
    pub const fn decimals() -> u8 { 18 }

    #[must_use]
    /// Minimum number of native tokens that can be transferred by `NeonEVM`
    pub fn min_transfer_value() -> U256 {
        let min_decimals: u32 = u32::from(decimals() - super::token_mint::decimals());
        10_u64.pow(min_decimals).into()
    }

    #[must_use]
    /// Cut down the remainder that can't be transferred
    pub fn round(value: U256) -> U256 {
        value - (value % min_transfer_value())
    }
}

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
    if account.data_is_empty() {
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

/// Validate Token Account Mint
/// 
/// # Errors
///
/// Will return: 
/// `ProgramError::IncorrectProgramId` if account is not token account
pub fn check_token_mint(token: &AccountInfo, mint: &Pubkey) -> Result<(), ProgramError> {
    debug_print!("check_token_mint");

    let token_data = get_token_account_data(&token.data.borrow(), token.owner)?;
    if token_data.mint == *mint {
        Ok(())
    } else {
        Err!(ProgramError::IncorrectProgramId; "token_data.mint<{}> == *mint<{}>", token_data.mint, mint)
    }
}


/// Transfer Tokens
/// 
/// # Errors
///
/// Could return: 
/// `ProgramError::InvalidInstructionData`
pub fn transfer_neon_token(
    accounts: &[AccountInfo],
    source_token_account: &AccountInfo,
    target_token_account: &AccountInfo,
    source_account: &AccountInfo,
    source_solidity_account: &SolidityAccount,
    value: &U256,
) -> Result<(), ProgramError> {
    debug_print!("transfer_neon_token");
    if get_token_account_owner(source_token_account)? != *source_account.key {
        return Err!(ProgramError::InvalidInstructionData;
            "Invalid account owner; source_token_account = {:?}, source_account = {:?}",
            source_token_account, source_account
        );
    }

    check_token_mint(source_token_account, &token_mint::id())?;
    check_token_mint(target_token_account, &token_mint::id())?;

    let value = value / eth::min_transfer_value();
    let value = u64::try_from(value).map_err(|_| E!(ProgramError::InvalidInstructionData))?;

    let source_token_balance = get_token_account_balance(source_token_account)?;
    if source_token_balance < value {
        return Err!(ProgramError::InvalidInstructionData;
            "Insufficient funds on token account {:?} {:?}",
            source_token_account, source_token_balance
        );
    }

    debug_print!("Transfer NEON tokens from {} to {} value {}", source_token_account.key, target_token_account.key, value);

    let instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_token_account.key,
        target_token_account.key,
        source_account.key,
        &[],
        value
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
pub fn user_pays_operator<'a>(
    user_token_account: &'a AccountInfo<'a>,
    operator_token_account: &'a AccountInfo<'a>,
    accounts: &'a [AccountInfo<'a>],
    account_storage: &ProgramAccountStorage,
    gas_limit: u64,
    gas_price: u64,
    paid_gas: u64,
    unpaid_gas: u64,
) -> Result<(), ProgramError> {

    if paid_gas + unpaid_gas > gas_limit {
        E!(EvmLoaderError::OutOfGas.into());
    }

    let gas_price_wei = U256::from(gas_price);

    let fee = U256::from(unpaid_gas)
        .checked_mul(gas_price_wei)
        .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

    transfer_neon_token(
        accounts,
        user_token_account,
        operator_token_account,
        account_storage.get_caller_account_info(),
        account_storage.get_caller_account(),
        &fee)?;

    Ok(())
}


/// Check that neon-evm user has enough funds to pay for gas
///
/// # Errors
///
/// Could return:
/// `ProgramError::InvalidArgument`
pub fn check_enough_funds<'a>(
    gas_limit: u64,
    gas_price: u64,
    user_token_account: &'a AccountInfo<'a>,
    storage_opt: Option<&mut StorageAccount>,
) -> Result<(), ProgramError> {

    let gas_used_and_paid = if let Some(storage) = storage_opt {
        storage.get_payments_info()?.0
    }
    else { 0 };

    let user_balance_64 = get_token_account_balance(user_token_account)?;
    let user_balance : U256 = U256::from(user_balance_64)
        .checked_mul(eth::min_transfer_value())
        .ok_or_else(|| E!(ProgramError::InvalidArgument))?;
    let gas_price_wei = U256::from(gas_price);
    let gas_to_be_paid = gas_limit.checked_sub(gas_used_and_paid)
        .ok_or_else(|| E!(ProgramError::InvalidArgument))?;
    let expected_fee = U256::from(gas_to_be_paid)
        .checked_mul(gas_price_wei)
        .ok_or_else(|| E!(ProgramError::InvalidArgument))?;

    if expected_fee > user_balance {
        return Err!(ProgramError::InsufficientFunds;
            "there is no enough funds to start executing the transaction; gas_limit = {:?}; gas_price = {:?}; gas_used_and_paid = {:?}; user_balance = {:?};",
            gas_limit,
            gas_price,
            gas_used_and_paid,
            user_balance_64
        )
    }
    Ok(())
}
