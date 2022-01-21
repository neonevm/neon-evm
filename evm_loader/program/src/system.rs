//! `EVMLoader` system functions
use solana_program::{
    account_info::AccountInfo,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar
};

/// Create program derived account
///
/// # Errors
///
/// Will return:
/// `ProgramError::AccountBorrowFailed` is `new_account` data already borrowed
/// `ProgramError::Custom` from `invoke_signed`
pub fn create_pda_account(
    owner: &Pubkey,
    accounts: &[AccountInfo],
    new_account: &AccountInfo,
    new_account_seeds: &[&[u8]],
    payer: &Pubkey,
    space: usize
) -> Result<(), ProgramError> {
    let rent = Rent::get()?;
    let minimum_balance = rent.minimum_balance(space).max(1);

    if new_account.lamports() > 0 {
        let required_lamports = minimum_balance.saturating_sub(new_account.lamports());

        if required_lamports > 0 {
            invoke(
                &system_instruction::transfer(payer, new_account.key, required_lamports),
                accounts
            )?;
        }

        invoke_signed(
            &system_instruction::allocate(new_account.key, space as u64),
            accounts,
            &[new_account_seeds],
        )?;

        invoke_signed(
            &system_instruction::assign(new_account.key, owner),
            accounts,
            &[new_account_seeds]
        )
    } else {
        invoke_signed(
            &system_instruction::create_account(
                payer,
                new_account.key,
                minimum_balance,
                space as u64,
                owner,
            ),
            accounts,
            &[new_account_seeds],
        )
    }
}
