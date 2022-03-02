use crate::account::{EthereumContract};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Update Valids Table");

    let contract = EthereumContract::from_account(program_id, &accounts[0])?;

    validate(&contract)?;
    execute(contract);

    Ok(())
}

fn validate(contract: &EthereumContract) -> ProgramResult {
    if contract.code_size == 0_u32 {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected ethereum contract", contract.info.key);
    }

    Ok(())
}

fn execute(mut contract: EthereumContract) {
    let valids = evm::Valids::compute(&contract.extension.code);
    contract.extension.valids.copy_from_slice(&valids);
}
