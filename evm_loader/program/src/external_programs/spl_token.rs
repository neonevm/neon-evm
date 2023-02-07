use std::collections::BTreeMap;

use crate::executor::{OwnedAccountInfo};
use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    account_info::{AccountInfo, IntoAccountInfo}, program_error::ProgramError, instruction::AccountMeta
};
use spl_token::instruction::TokenInstruction;


pub fn emulate(instruction: &[u8], meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>) -> ProgramResult {
    let accounts_info = accounts.iter_mut()
        .map(|(key, a)| (*key, a.into_account_info()))
        .collect::<BTreeMap<Pubkey, AccountInfo>>();

    let instruction_accounts: Vec<AccountInfo> = meta.iter().map(|a| {
        let mut info = accounts_info[&a.pubkey].clone();
        info.is_writable = a.is_writable;
        info.is_signer = a.is_signer;
        info
    }).collect();
    
    // spl_token::processor::Processor::process(&spl_token::ID, &instruction_accounts, instruction)
    let instruction = TokenInstruction::unpack(instruction)?;

    match instruction {
        TokenInstruction::InitializeMint { decimals, mint_authority, freeze_authority } => {
            spl_token::processor::Processor::process_initialize_mint(&instruction_accounts, decimals, mint_authority, freeze_authority)
        }
        TokenInstruction::InitializeMint2 { decimals, mint_authority, freeze_authority } => {
            spl_token::processor::Processor::process_initialize_mint2(&instruction_accounts, decimals, mint_authority, freeze_authority)
        }
        TokenInstruction::InitializeAccount => {
            spl_token::processor::Processor::process_initialize_account(&spl_token::ID, &instruction_accounts)
        }
        TokenInstruction::InitializeAccount2 { owner } => {
            spl_token::processor::Processor::process_initialize_account2(&spl_token::ID, &instruction_accounts, owner)
        }
        TokenInstruction::InitializeAccount3 { owner } => {
            spl_token::processor::Processor::process_initialize_account3(&spl_token::ID, &instruction_accounts, owner)
        }
        TokenInstruction::InitializeMultisig { m } => {
            spl_token::processor::Processor::process_initialize_multisig(&instruction_accounts, m)
        }
        TokenInstruction::InitializeMultisig2 { m } => {
            spl_token::processor::Processor::process_initialize_multisig2(&instruction_accounts, m)
        }
        TokenInstruction::Transfer { amount } => {
            spl_token::processor::Processor::process_transfer(&spl_token::ID, &instruction_accounts, amount, None)
        }
        TokenInstruction::Approve { amount } => {
            spl_token::processor::Processor::process_approve(&spl_token::ID, &instruction_accounts, amount, None)
        }
        TokenInstruction::Revoke => {
            spl_token::processor::Processor::process_revoke(&spl_token::ID, &instruction_accounts)
        }
        TokenInstruction::SetAuthority { authority_type, new_authority } => {
            spl_token::processor::Processor::process_set_authority(&spl_token::ID, &instruction_accounts, authority_type, new_authority)
        }
        TokenInstruction::MintTo { amount } => {
            spl_token::processor::Processor::process_mint_to(&spl_token::ID, &instruction_accounts, amount, None)
        }
        TokenInstruction::Burn { amount } => {
            spl_token::processor::Processor::process_burn(&spl_token::ID, &instruction_accounts, amount, None)
        }
        TokenInstruction::CloseAccount => {
            spl_token::processor::Processor::process_close_account(&spl_token::ID, &instruction_accounts)
        }
        TokenInstruction::FreezeAccount => {
            spl_token::processor::Processor::process_toggle_freeze_account(&spl_token::ID, &instruction_accounts, true)
        }
        TokenInstruction::ThawAccount => {
            spl_token::processor::Processor::process_toggle_freeze_account(&spl_token::ID, &instruction_accounts, false)
        }
        TokenInstruction::TransferChecked { amount, decimals } => {
            spl_token::processor::Processor::process_transfer(&spl_token::ID, &instruction_accounts, amount, Some(decimals))
        }
        TokenInstruction::ApproveChecked { amount, decimals } => {
            spl_token::processor::Processor::process_approve(&spl_token::ID, &instruction_accounts, amount, Some(decimals))
        }
        TokenInstruction::MintToChecked { amount, decimals } => {
            spl_token::processor::Processor::process_mint_to(&spl_token::ID, &instruction_accounts, amount, Some(decimals))
        }
        TokenInstruction::BurnChecked { amount, decimals } => {
            spl_token::processor::Processor::process_burn(&spl_token::ID, &instruction_accounts, amount, Some(decimals))
        }
        TokenInstruction::SyncNative => {
            spl_token::processor::Processor::process_sync_native(&spl_token::ID, &instruction_accounts)
        }
        _ => {
            Err!(ProgramError::InvalidInstructionData; "SPL Token: unknown instrtuction")
        }
    }
}