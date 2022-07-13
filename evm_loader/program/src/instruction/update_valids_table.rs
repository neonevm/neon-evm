use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::account::{ether_contract, EthereumAccount};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Update Valids Table");

    let mut account = EthereumAccount::from_account(program_id, &accounts[0])?;

    validate(&account)?;
    execute(account.extension.as_mut().unwrap());

    Ok(())
}

fn validate(account: &EthereumAccount) -> ProgramResult {
    if account.extension.is_none() {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected ethereum contract", account.info.key);
    }

    Ok(())
}

fn execute(contract: &mut ether_contract::Extension) {
    let valids = evm::Valids::compute(&contract.code);
    contract.valids.copy_from_slice(&valids);
}
