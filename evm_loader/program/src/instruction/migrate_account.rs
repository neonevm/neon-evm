use crate::account::{self, program, token, EthereumAccountV1, EthereumAccount, Operator};
use crate::config::token_mint;

use spl_associated_token_account::get_associated_token_address;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg
};

struct Accounts<'a> {
    operator: Operator<'a>,
    ethereum_account: EthereumAccountV1<'a>,
    token_balance_account: token::State<'a>,
    token_pool_account: token::State<'a>,
    authority_info: &'a AccountInfo<'a>,
    token_program: program::Token<'a>,
}

/// Processes the migration of an Ethereum account to the current version.
pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    msg!("Instruction: MigrateAccount");

    let parsed_accounts = Accounts {
        operator: Operator::from_account(&accounts[0])?,
        ethereum_account: EthereumAccountV1::from_account(program_id, &accounts[1])?,
        token_balance_account: token::State::from_account(&accounts[2])?,
        token_pool_account: token::State::from_account(&accounts[3])?,
        authority_info: &accounts[4],
        token_program: program::Token::from_account(&accounts[5])?,
    };

    validate(program_id, &parsed_accounts)?;
    execute(&parsed_accounts)?;

    Ok(())
}

/// Checks incoming accounts.
fn validate(program_id: &Pubkey, accounts: &Accounts) -> ProgramResult {
    msg!("MigrateAccount: validate");

    let (expected_address, _) = Pubkey::find_program_address(&[b"Deposit"], program_id);
    if accounts.authority_info.key != &expected_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected PDA address {}",
            accounts.authority_info.key, expected_address);
    }

    let expected_pool_address = get_associated_token_address(
        accounts.authority_info.key,
        &token_mint::id()
    );
    if accounts.token_pool_account.info.key != &expected_pool_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token Pool {}",
            accounts.token_pool_account.info.key, expected_pool_address);
    }

    if accounts.ethereum_account.rw_blocked_acc.is_some()
        || accounts.ethereum_account.ro_blocked_cnt > 0 {
        return Err!(ProgramError::InvalidInstructionData;
            "Account {} - is blocked",
            accounts.ethereum_account.ether);
    }

    Ok(())
}

/// Executes all actions.
fn execute(accounts: &Accounts) -> ProgramResult {
    msg!("MigrateAccount: execute");
    let amount = accounts.token_balance_account.amount;

    msg!("MigrateAccount: convert_from_v1");
    let ethereum_account = EthereumAccount::convert_from_v1(
        &accounts.ethereum_account,
        amount)?;

    msg!("MigrateAccount: approve");
    accounts.token_program.approve(
        &ethereum_account,
        accounts.token_balance_account.info,
        accounts.authority_info,
        amount)?;

    msg!("MigrateAccount: transfer");
    accounts.token_program.transfer(
        &ethereum_account,
        accounts.token_balance_account.info,
        accounts.token_pool_account.info,
        amount)?;

    unsafe {
        account::delete(accounts.token_balance_account.info,
                        &accounts.operator)?;
    }

    msg!("MigrateAccount: OK");
    Ok(())
}
