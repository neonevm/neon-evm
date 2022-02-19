use crate::account::{token, EthereumAccountV1, EthereumAccount};
use crate::config::token_mint;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey, msg
};

use spl_associated_token_account::get_associated_token_address;

struct Accounts<'a> {
    signer: &'a AccountInfo<'a>,
    ethereum_account: EthereumAccountV1<'a>,
    token_balance_account: token::State<'a>,
}

/// Processes the migration of an Ethereum account to current version.
pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    msg!("Instruction: MigrateAccount");

    let parsed_accounts = Accounts {
        signer: &accounts[0],
        ethereum_account: EthereumAccountV1::from_account(program_id, &accounts[1])?,
        token_balance_account: token::State::from_account(&accounts[2])?,
    };

    validate(&parsed_accounts)?;
    execute(&parsed_accounts)?;

    Ok(())
}

fn validate(accounts: &Accounts) -> ProgramResult {
    if !accounts.signer.is_signer {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected signer",
            accounts.signer.key);
    }

    if accounts.ethereum_account.rw_blocked_acc.is_some()
        || accounts.ethereum_account.ro_blocked_cnt > 0 {
        return Err!(ProgramError::InvalidInstructionData;
            "Account {} - is blocked",
            accounts.ethereum_account.ether);
    }

    if accounts.token_balance_account.mint != token_mint::id() {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token account",
            accounts.token_balance_account.info.key);
    }

    let expected_token_account = get_associated_token_address(
        accounts.ethereum_account.info.key,
        &token_mint::id()
    );
    if accounts.token_balance_account.info.key != &expected_token_account {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token Account {} associated with Ethereum address {}",
            accounts.token_balance_account.info.key,
            expected_token_account,
            accounts.ethereum_account.ether);
    }

    Ok(())
}

fn execute(accounts: &Accounts) -> ProgramResult {
    convert_ether_account(&accounts.ethereum_account,
                          accounts.token_balance_account.amount)?;

    transfer_tokens_to_pool()?;

    delete_token_account()
}

fn convert_ether_account(v1: &EthereumAccountV1, balance: u64) -> ProgramResult {
    let v2 = EthereumAccount::from_v1(v1, balance)?;
    dbg!(&v2);
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
const fn transfer_tokens_to_pool() -> ProgramResult {
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
const fn delete_token_account() -> ProgramResult {
    Ok(())
}
