use std::collections::BTreeMap;

use crate::executor::{OwnedAccountInfo};
use borsh::BorshDeserialize;
use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError, rent::Rent, sysvar::Sysvar, program_pack::Pack, instruction::AccountMeta
};
use spl_associated_token_account::instruction::AssociatedTokenAccountInstruction;


pub fn emulate(instruction: &[u8], meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>) -> ProgramResult {
    let instruction = if instruction.is_empty() {
        AssociatedTokenAccountInstruction::Create
    } else {
        AssociatedTokenAccountInstruction::try_from_slice(instruction)
            .map_err(|_| ProgramError::InvalidInstructionData)?
    };
    
    if instruction != AssociatedTokenAccountInstruction::Create {
        return Err!(ProgramError::InvalidInstructionData; "Unknown spl_associated_token instruction");
    }

    let funder_key = &meta[0].pubkey;
    let associated_token_account_key = &meta[1].pubkey;
    let wallet_account_key = &meta[2].pubkey;
    let spl_token_mint_key = &meta[3].pubkey;
    // let system_program_key = &meta[4].pubkey;
    let spl_token_program_key = &meta[5].pubkey;

    let required_lamports = {
        let associated_token_account = &accounts[associated_token_account_key];

        let rent = Rent::get()?;
        rent.minimum_balance(spl_token::state::Account::LEN)
            .max(1)
            .saturating_sub(associated_token_account.lamports)
    };

    {
        let mut funder = accounts.get_mut(funder_key).unwrap();
        if funder.lamports < required_lamports {
            return Err!(ProgramError::InsufficientFunds; "Insufficient operator lamports");
        }

        funder.lamports -= required_lamports;
    }
    
    {
        let mut associated_token_account = accounts.get_mut(associated_token_account_key).unwrap();
        if !solana_program::system_program::check_id(&associated_token_account.owner) {
            return Err!(ProgramError::InvalidInstructionData; "Account {} is not system owned", associated_token_account_key);
        }
        
        associated_token_account.lamports += required_lamports;
        associated_token_account.owner = spl_token::ID;
        associated_token_account.data.resize(spl_token::state::Account::LEN, 0);
    }


    let initialize_account = spl_token::instruction::initialize_account3(
        spl_token_program_key,
        associated_token_account_key,
        spl_token_mint_key,
        wallet_account_key,
    )?;

    let instruction: &[u8] = &initialize_account.data;
    let meta: Vec<AccountMeta> = initialize_account.accounts;
    super::spl_token::emulate(instruction, &meta, accounts)
}