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
    if instruction_data.len() > 0 && instruction_data[0] > 0xee {
        for _ in 0..(instruction_data[0] - 0xee) {
            let instruction = Instruction {
                program_id: *accounts[0].key,
                accounts: accounts[1..].iter().map(|acc| 
                    if acc.is_writable {
                        AccountMeta::new(*acc.key, acc.is_signer)
                    } else {
                        AccountMeta::new_readonly(*acc.key, acc.is_signer)
                    }
                ).collect(),
                data: instruction_data[1..].to_vec()
            };
            invoke(
                &instruction,
                &accounts[1..],
            )?;
        }
        return  Ok(());
    } else {
        let instruction = Instruction {
            program_id: *accounts[0].key,
            accounts: accounts[1..].iter().map(|acc| 
                if acc.is_writable {
                    AccountMeta::new(*acc.key, acc.is_signer)
                } else {
                    AccountMeta::new_readonly(*acc.key, acc.is_signer)
                }
            ).collect(),
            data: instruction_data.to_vec()
        };
        invoke(
            &instruction,
            &accounts[1..],
        )
    }
}
