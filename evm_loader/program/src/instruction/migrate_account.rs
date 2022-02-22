use crate::account::{program, token, EthereumAccountV1, EthereumAccount};
use crate::config::token_mint;

use spl_associated_token_account::get_associated_token_address;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg
};

use solana_program::program::{invoke_signed};

struct Accounts<'a> {
    signer_info: &'a AccountInfo<'a>,
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
        signer_info: &accounts[0],
        ethereum_account: EthereumAccountV1::from_account(program_id, &accounts[1])?,
        token_balance_account: token::State::from_account(&accounts[2])?,
        token_pool_account: token::State::from_account(&accounts[3])?,
        authority_info: &accounts[4],
        token_program: program::Token::from_account(&accounts[5])?,
    };

    let bump_seed = validate(program_id, &parsed_accounts)?;
    execute(&parsed_accounts, bump_seed)?;

    Ok(())
}

/// Checks incoming accounts.
fn validate(program_id: &Pubkey, accounts: &Accounts) -> Result<u8, ProgramError> {
    msg!("MigrateAccount: validate");

    if !accounts.signer_info.is_signer {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected signer",
            accounts.signer_info.key);
    }

    let (expected_address, bump_seed) = Pubkey::find_program_address(&[b"Deposit"], program_id);
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

    Ok(bump_seed)
}

/// Executes all actions.
fn execute(accounts: &Accounts, _bump_seed: u8) -> ProgramResult {
    msg!("MigrateAccount: execute");

    msg!("MigrateAccount: convert_from_v1");
    let ethereum_account = EthereumAccount::convert_from_v1(
        &accounts.ethereum_account,
        accounts.token_balance_account.amount)?;

    msg!("MigrateAccount: approve");
    accounts.token_program.approve(
        &ethereum_account,
        accounts.token_balance_account.info,
        accounts.authority_info,
        accounts.token_balance_account.amount,
    )?;

    msg!("MigrateAccount: transfer_tokens_to_pool");
    //transfer_tokens_to_pool(accounts, bump_seed)?;
    accounts.token_program.transfer(
        &ethereum_account,
        accounts.token_balance_account.info,
        accounts.token_pool_account.info,
        accounts.token_balance_account.amount,
    )?;

    //delete_account(accounts.token_balance_account.info);

    Ok(())
}

/// Transfers all funds from old balance account to the pool account.
#[allow(unused)]
fn transfer_tokens_to_pool(accounts: &Accounts, bump_seed: u8) -> ProgramResult {
    msg!("==== from address {:?}", &accounts.token_balance_account.info);
    msg!("==== from owner {:?}", &accounts.token_balance_account.owner);
    msg!("==== to address {:?}", &accounts.token_pool_account.info);
    msg!("==== to owner {:?}", &accounts.token_pool_account.owner);

    let signers_seeds: &[&[&[u8]]] = &[&[b"Deposit", &[bump_seed]]];

    let instruction = spl_token::instruction::transfer(
        accounts.token_program.key,
        accounts.token_balance_account.info.key,
        accounts.token_pool_account.info.key,
        accounts.authority_info.key,
        &[],
        accounts.token_balance_account.amount
    )?;

    let account_infos: &[AccountInfo] = &[
        accounts.token_balance_account.info.clone(),
        accounts.token_pool_account.info.clone(),
        accounts.authority_info.clone(),
        accounts.token_program.clone(),
    ];

    invoke_signed(&instruction, account_infos, signers_seeds)?;

    Ok(())
}

/// Permanently deletes all data in the account.
#[allow(unused)]
fn delete_account(account: &AccountInfo) {
    msg!("DELETE ACCOUNT {}", account.key);
    **account.lamports.borrow_mut() = 0;
    let mut data = account.data.borrow_mut();
    data.fill(0);
}
