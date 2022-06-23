use std::collections::BTreeMap;

use crate::executor::{OwnedAccountInfo, AccountMeta};
use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    account_info::{AccountInfo, IntoAccountInfo}
};


pub fn emulate(instruction: &[u8], meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>) -> ProgramResult {
    let accounts_info = accounts.iter_mut()
        .map(|(key, a)| (*key, a.into_account_info()))
        .collect::<BTreeMap<Pubkey, AccountInfo>>();

    let instruction_accounts: Vec<AccountInfo> = meta.iter().map(|a| {
        let mut info = accounts_info[&a.key].clone();
        info.is_writable = a.is_writable;
        info.is_signer = a.is_signer;
        info
    }).collect();
    
    spl_token::processor::Processor::process(&spl_token::ID, &instruction_accounts, instruction)
}