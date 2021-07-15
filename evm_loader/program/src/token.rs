//! `EVMLoader` token functions

use solana_program::{
    instruction::{AccountMeta, Instruction},
    account_info::{AccountInfo},
    pubkey::Pubkey,
    system_program, sysvar,
    program_error::ProgramError,
    program_pack::Pack,
};
use std::vec;

/// Token Mint ID
pub mod token_mint {
    solana_program::declare_id!("HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");

    /// Number of base 10 digits to the right of the decimal place
    #[must_use]
    pub const fn decimals() -> u8 { 9 }
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
    if *account.owner != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let data = spl_token::state::Account::unpack(&account.data.borrow())?;
    Ok(data.amount)
}