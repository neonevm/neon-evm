use crate::account::{Treasury, MainTreasury};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
    rent::Rent,
};

//struct Accounts<'a> {
//    main_treasury: MainTreasury<'a>,
//    treasury: Treasury<'a>,
//    //system_program: program::System<'a>,
//}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Collect treasury");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);

    let main_treasury = MainTreasury::from_account(&accounts[0])?;
    let treasury = Treasury::from_account(program_id, treasury_index, &accounts[1])?;
        //system_program: program::System::from_account(&accounts[2])?,

    let rent = Rent::get()?;
    let minimal_balance_for_rent_exempt = rent.minimum_balance(treasury.data_len());
    let available_lamports = treasury.lamports().saturating_sub(minimal_balance_for_rent_exempt);

    **treasury.lamports.borrow_mut() = minimal_balance_for_rent_exempt;
    **main_treasury.lamports.borrow_mut() = main_treasury.lamports().checked_add(available_lamports)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Main treasary lamports overflow"))?;

    //accounts.system_program.transfer(&accounts.treasury, &accounts.main_treasury, available_lamports)?;

    //super::transaction::do_continue(step_count, accounts, storage, &mut account_storage)
    Ok(())
}
