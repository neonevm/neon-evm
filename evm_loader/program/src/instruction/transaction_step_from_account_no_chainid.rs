use crate::error::Result;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: Begin or Continue Transaction from Account Without ChainId");

    super::transaction_step_from_account::process_inner(program_id, accounts, instruction, true)
}
