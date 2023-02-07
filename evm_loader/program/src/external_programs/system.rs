#![allow(clippy::cast_possible_truncation)]

use std::collections::BTreeMap;

use crate::executor::{OwnedAccountInfo};
use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_instruction::SystemInstruction,
    program_error::ProgramError, system_program, instruction::AccountMeta
};


pub fn emulate(instruction: &[u8], meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>) -> ProgramResult {
    let system_instruction: SystemInstruction = bincode::deserialize(instruction).unwrap();
    match system_instruction {
        SystemInstruction::CreateAccount { lamports, space, owner } => {
            let funder_key = &meta[0].pubkey;
            let account_key = &meta[1].pubkey;
            
            {
                let mut funder = accounts.get_mut(funder_key).unwrap();
                if funder.lamports < lamports {
                    return Err!(ProgramError::InsufficientFunds; "Insufficient operator lamports");
                }

                funder.lamports -= lamports;
            }

            {
                let mut account = accounts.get_mut(account_key).unwrap();
                if (account.lamports > 0) || !account.data.is_empty() || !system_program::check_id(&account.owner) {
                    return Err!(ProgramError::InvalidInstructionData; "Create Account: account already in use");
                }

                account.lamports = lamports;
                account.owner = owner;
                account.data.resize(space as usize, 0_u8);
            }
        },
        SystemInstruction::Assign { owner } => {
            let account_key = &meta[0].pubkey;
            let mut account = accounts.get_mut(account_key).unwrap();

            if !system_program::check_id(&account.owner) {
                return Err!(ProgramError::InvalidInstructionData; "Assign Account: account already in use");
            }

            account.owner = owner;
        },
        SystemInstruction::Transfer { lamports } => {
            let from_key = &meta[0].pubkey;
            let to_key = &meta[1].pubkey;

            {
                let mut from = accounts.get_mut(from_key).unwrap();
                if !from.data.is_empty() {
                    return Err!(ProgramError::InvalidArgument; "Transfer: `from` must not carry data");
                }

                if from.lamports < lamports {
                    return Err!(ProgramError::InsufficientFunds; "Transfer: insufficient lamports");
                }

                if !system_program::check_id(&from.owner) {
                    return Err!(ProgramError::InsufficientFunds; "Transfer: source is not system owned");
                }

                from.lamports -= lamports;
            }

            {
                let mut to = accounts.get_mut(to_key).unwrap();
                to.lamports += lamports;
            }
        },
        SystemInstruction::Allocate { space } => {
            let account_key = &meta[0].pubkey;
            let account = accounts.get_mut(account_key).unwrap();

            if !account.data.is_empty() || !system_program::check_id(&account.owner) {
                return Err!(ProgramError::InvalidInstructionData; "Allocate Account: account already in use");
            }

            account.data.resize(space as usize, 0_u8);
        },
        _ => {
            return Err!(ProgramError::InvalidInstructionData; "Unknown system instruction");
        }
    }

    Ok(())
}