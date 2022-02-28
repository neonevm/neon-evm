use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey
};
use spl_associated_token_account::get_associated_token_address;

use crate::account::{EthereumAccount, EthereumAccountV1, Operator, program, token};
use crate::config::token_mint;

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
    debug_print!("MigrateAccount: validate");

    if &accounts.ethereum_account.eth_token_account !=
       accounts.token_balance_account.info.key {
        return Err!(ProgramError::InvalidArgument;
            "Ethereum account V1 {} should store balance in {} - got {}",
            &accounts.ethereum_account.ether,
            &accounts.ethereum_account.eth_token_account,
            accounts.token_balance_account.info.key);
    }

    let (expected_address, _) = Pubkey::find_program_address(&[b"Deposit"], program_id);
    if accounts.authority_info.key != &expected_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected PDA address {}",
            accounts.authority_info.key,
            expected_address);
    }

    let expected_pool_address = get_associated_token_address(
        accounts.authority_info.key,
        &token_mint::id()
    );
    if accounts.token_pool_account.info.key != &expected_pool_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token Pool {}",
            accounts.token_pool_account.info.key,
            expected_pool_address);
    }

    Ok(())
}

/// Executes all actions.
fn execute(accounts: &Accounts) -> ProgramResult {
    debug_print!("MigrateAccount: execute");
    let amount = accounts.token_balance_account.amount;

    debug_print!("MigrateAccount: convert_from_v1");
    let ethereum_account = EthereumAccount::convert_from_v1(
        &accounts.ethereum_account,
        scale(amount)?)?;

    debug_print!("MigrateAccount: transfer");
    accounts.token_program.transfer(
        &ethereum_account,
        accounts.token_balance_account.info,
        accounts.token_pool_account.info,
        amount)?;

    debug_print!("MigrateAccount: close token account");
    accounts.token_program.close_account(
        &ethereum_account,
        accounts.token_balance_account.info,
        &accounts.operator)?;

    debug_print!("MigrateAccount: OK");
    Ok(())
}

use evm::U256;

/// Recalculates amount from decimals 10^9 to 10^18.
/// Neon token amount is SPL token. It's decimals is 10^9.
/// `EthereumAccount` stores balance with decimals 10^18.
/// We need to convert amount to `U256` and multiply by 10^9
/// before assigning it to `EthereumAccount::balance`.
fn scale(amount: u64) -> Result<U256, ProgramError> {
    assert!(token_mint::decimals() <= 18);
    let additional_decimals: u32 = (18 - token_mint::decimals()).into();
    U256::from(amount).checked_mul(U256::from(10_u64.pow(additional_decimals)))
        .ok_or_else(|| E!(ProgramError::InvalidArgument;
            "Amount {} scale overflow",
            amount))
}
