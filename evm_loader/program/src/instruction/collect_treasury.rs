use crate::{
    account::{Treasury, MainTreasury, program::System},
    config::TREASURY_POOL_SEED,
};
use arrayref::{array_ref};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
    program::invoke_signed,
    system_instruction,
    sysvar::Sysvar,
    rent::Rent,
};

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Collect treasury");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);

    let main_treasury = MainTreasury::from_account(program_id, &accounts[0])?;
    let treasury = Treasury::from_account(program_id, treasury_index, &accounts[1])?;
    let system = System::from_account(&accounts[2])?;

    let rent = Rent::get()?;
    let minimal_balance_for_rent_exempt = rent.minimum_balance(treasury.data_len());
    let available_lamports = treasury.lamports().saturating_sub(minimal_balance_for_rent_exempt);

    if available_lamports > 0 {
        invoke_signed(
            &system_instruction::transfer(
                treasury.key,
                main_treasury.key,
                available_lamports
            ),
            &[treasury.clone(), main_treasury.clone(), system.clone()],
            &[&[TREASURY_POOL_SEED.as_bytes(), &treasury_index.to_le_bytes(), &[treasury.get_bump_seed()]]]
        )?;
    };

    Ok(())
}
