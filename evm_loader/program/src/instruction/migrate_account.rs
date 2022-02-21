use crate::account::{program, token, EthereumAccountV1, EthereumAccount};
use crate::config::token_mint;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg
};

use solana_program::program::invoke;

struct Accounts<'a> {
    signer: &'a AccountInfo<'a>,
    ethereum_account: EthereumAccountV1<'a>,
    token_balance_account: token::State<'a>,
    token_pool_account: token::State<'a>,
    token_program: program::Token<'a>,
}

/// Processes the migration of an Ethereum account to current version.
pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    msg!("Instruction: MigrateAccount");

    let parsed_accounts = Accounts {
        signer: &accounts[0],
        ethereum_account: EthereumAccountV1::from_account(program_id, &accounts[1])?,
        token_balance_account: token::State::from_account(&accounts[2])?,
        token_pool_account: token::State::from_account(&accounts[3])?,
        token_program: program::Token::from_account(&accounts[4])?,
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

    if accounts.token_pool_account.mint != token_mint::id() {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token account",
            accounts.token_pool_account.info.key);
    }

    /* Need this? get_associated_token_address is a costly function...
    use spl_associated_token_account::get_associated_token_address;
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
    }*/

    Ok(())
}

fn execute(accounts: &Accounts) -> ProgramResult {
    EthereumAccount::convert_from_v1(
        &accounts.ethereum_account,
        accounts.token_balance_account.amount)?;

    transfer_tokens_to_pool(accounts)?;

    delete_token_account()
}

fn transfer_tokens_to_pool(accounts: &Accounts) -> ProgramResult {
    let instruction = spl_token::instruction::transfer(
        accounts.token_program.key,
        accounts.token_balance_account.info.key,
        accounts.token_pool_account.info.key,
        accounts.signer.key,
        &[],
        accounts.token_balance_account.amount
    )?;

    let account_infos: &[AccountInfo] = &[
        accounts.token_balance_account.info.clone(),
        accounts.token_pool_account.info.clone(),
        accounts.signer.clone(),
        accounts.token_program.clone(),
    ];

    invoke(&instruction, account_infos)?;
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
const fn delete_token_account() -> ProgramResult {
    Ok(())
}
