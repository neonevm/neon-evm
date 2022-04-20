//! Program entrypoint

#![cfg(not(feature = "no-entrypoint"))]

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
    program::invoke,
};

entrypoint!(process_instruction);
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = Instruction {
        program_id: *accounts[0].key,
        accounts: accounts[1..].iter().map(|acc| AccountMeta::new(*acc.key, acc.is_signer)).collect(),
        data: instruction_data.to_vec()
    };
    invoke(
        &instruction,
        &accounts[1..],
    )
}
