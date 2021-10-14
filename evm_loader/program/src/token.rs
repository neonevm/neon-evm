//! `EVMLoader` token functions
use crate::{
    account_data::{
        AccountData,
    },
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
    entrypoint::ProgramResult,
    sysvar::{rent::Rent,Sysvar},
    program::invoke,
    system_instruction,
};
use std::vec;
use std::convert::TryFrom;

use crate::neon::token_mint;
use crate::account_data::{
    ACCOUNT_SEED_VERSION,
    BLOCKING_TOKEN_ACCOUNT_SEED_VERSION
};


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

/// Create an blocking token account for the given wallet address and token mint
///
/// # Errors
///
pub fn create_blocking_token_account<'a>(
    funding_info: &'a AccountInfo<'a>,
    wallet_info: &'a AccountInfo<'a>,
    token_account_info: &'a AccountInfo<'a>,
    system_program_info: &'a AccountInfo<'a>,
    spl_token_mint_info: &'a AccountInfo<'a>,
    spl_token_program_info: &'a AccountInfo<'a>,
    rent_sysvar_info: &'a AccountInfo<'a>,
) -> ProgramResult {

    let token_account_seeds: &[&[_]] = &[
        &BLOCKING_TOKEN_ACCOUNT_SEED_VERSION.to_le_bytes(),
        &wallet_info.key.to_bytes(),
        &spl_token_program_info.key.to_bytes(),
        &spl_token_mint_info.key.to_bytes(),
    ];

    // Fund the associated token account with the minimum balance to be rent exempt
    let rent = Rent::get()?;
    let required_lamports = rent
        .minimum_balance(spl_token::state::Account::LEN)
        .max(1)
        .saturating_sub(token_account_info.lamports());

    if required_lamports > 0 {
        debug_print!(
            "Transfer {} lamports to the blocking token account",
            required_lamports
        );

        invoke(
            &system_instruction::transfer(funding_info.key,
                                          token_account_info.key,
                                          required_lamports),
            &[
                funding_info.clone(),
                token_account_info.clone(),
                system_program_info.clone(),
            ],
        )?;
    }

    debug_print!("Allocate space for the blocking token account");
    invoke_signed(
        &system_instruction::allocate(
            token_account_info.key,
            spl_token::state::Account::LEN as u64,
        ),
        &[
            token_account_info.clone(),
            system_program_info.clone(),
        ],
        &[token_account_seeds],
    )?;

    debug_print!("Assign the blocking token account to the SPL Token program");
    invoke_signed(
        &system_instruction::assign(token_account_info.key,
                                    spl_token_program_info.key),
        &[
            token_account_info.clone(),
            system_program_info.clone(),
        ],
        &[token_account_seeds],
    )?;

    debug_print!("Initialize the blocking token account");
    invoke(
        &spl_token::instruction::initialize_account(
            spl_token_program_info.key,
            token_account_info.key,
            spl_token_mint_info.key,
            wallet_info.key,
        )?,
        &[
            token_account_info.clone(),
            spl_token_mint_info.clone(),
            wallet_info.clone(),
            rent_sysvar_info.clone(),
            spl_token_program_info.clone(),
        ],
    )
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
        return Err!(ProgramError::IncorrectProgramId; "Invalid account owner; *account.owner != spl_token::id(); *account.owner={:?}; spl_token::id()={:?}", *account.owner, spl_token::id());
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
        return Err!(ProgramError::IncorrectProgramId; "token.owner != spl_token::id(); token.owner={:?}; spl_token::id()={:?}", token.owner, spl_token::id());
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

/// Make an blocking token account address and nonce for the given wallet address and token mint
#[must_use]
pub fn make_blocking_token_address(
    wallet_address: &Pubkey,
    spl_token_mint_address: &Pubkey
) -> (Pubkey, u8)  {
    Pubkey::find_program_address(
        &[
            &BLOCKING_TOKEN_ACCOUNT_SEED_VERSION.to_le_bytes(),
            &wallet_address.to_bytes(),
            &spl_token::id().to_bytes(),
            &spl_token_mint_address.to_bytes(),
        ],
        &spl_token::id(),
    )
}

/// get an blocking token account address for the given wallet address and token mint
#[must_use]
pub fn get_blocking_token_address(
    wallet_address: &Pubkey,
    spl_token_mint_address: &Pubkey
) -> Pubkey {
    make_blocking_token_address(
        wallet_address,
        spl_token_mint_address,
    ).0
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

    let source_token_balance = get_token_account_balance(source_token_account)?;
    if source_token_balance < value {
        return Err!(ProgramError::InvalidInstructionData; "Insufficient funds on token account {} {}", source_token_account.key, source_token_balance)
    }

    debug_print!("Transfer NEON tokens from {} to {} value {}", source_token_account.key, target_token_account.key, value);

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
    let pda_ether_seeds : &[&[u8]] = &[
        &[ACCOUNT_SEED_VERSION],
        ether.as_bytes(),
        &[nonce]
    ];
    let blocking_seed = source_solidity_account.get_blocking_seed();
    let blocking_token_seeds: &[&[u8]] = &[
        &BLOCKING_TOKEN_ACCOUNT_SEED_VERSION.to_le_bytes(),
        &source_solidity_account.get_solana_address().to_bytes(),
        &spl_token::id().to_bytes(),
        &crate::neon::token_mint::id().to_bytes(),
        &[blocking_seed],
    ];

    invoke_signed(
        &instruction,
        accounts,
        &[pda_ether_seeds, blocking_token_seeds]
    )?;

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
    // let (ether, _nonce) = source_solidity_account.get_seeds();
    debug_print!("block_token from={:?}; to={:?}; source={:?}; source_solidity={:?}; value={:?}",
        source_token_account, target_token_account, source_account, source_solidity_account, value);

    let user_token_account = spl_associated_token_account::get_associated_token_address(source_account.key, &token_mint::id());
    debug_print!("source_token_account={:?}; user_token_account={:?}",*source_token_account.key, user_token_account);
    if *source_token_account.key != user_token_account {
        return Err!(ProgramError::InvalidInstructionData;
            "Invalid user token account: source_token_account != user_token_account; source_token_account={:?}; user_token_account={:?}",
            *source_token_account.key, user_token_account)
    }

    // let target_token_account_owner = get_token_account_owner(target_token_account)?;
    // debug_print!("target_token_account_owner={:?}; source_account.key={:?}",
    //     target_token_account_owner, *source_account.key);
    // if target_token_account_owner != *source_account.key {
    //     return Err!(ProgramError::InvalidInstructionData;
    //         "Invalid target token account owner: target_token_account_owner != source_account; target_token_account_owner={:?}; source_account={:?}",
    //         target_token_account_owner, *source_account.key)
    // }

    // let blocking_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "blocking_account";
    let blocking_seed = "blocking_account";
    // debug_print!("ether={:?}; blocking_seed={:?}", ether, blocking_seed);
    let blocking_address = Pubkey::create_with_seed(source_account.key, blocking_seed, &spl_token::id())?;
    let blocking_token_account = spl_associated_token_account::get_associated_token_address(&blocking_address, &token_mint::id());
    debug_print!("target_token_account={:?}; blocking_address={:?}; blocking_token_account={:?}", blocking_address, blocking_token_account, *target_token_account.key);
    if *target_token_account.key != blocking_token_account {
        return Err!(ProgramError::InvalidInstructionData;
            "Invalid blocking token account: target_token_account != blocking_token_account; target_token_account={:?}; blocking_token_account={:?}",
            *target_token_account.key, blocking_token_account)
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
    let holder_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "blocking_account";
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
    let holder_seed = bs58::encode(&ether.to_fixed_bytes()).into_string() + "blocking_account";
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