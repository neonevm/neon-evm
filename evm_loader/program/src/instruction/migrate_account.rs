use crate::account::{token, EthereumAccountV1, EthereumAccount};
use crate::config::token_mint;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg
};

use solana_program::program::invoke;

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
    execute(program_id, &parsed_accounts)?;

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

fn execute(program_id: &Pubkey, accounts: &Accounts) -> ProgramResult {
    EthereumAccount::convert_from_v1(
        &accounts.ethereum_account,
        accounts.token_balance_account.amount)?;

    transfer_tokens_to_pool(
        program_id,
        accounts.token_balance_account.amount,
        &[accounts.token_balance_account.info,
          accounts.signer],
    )?;

    delete_token_account()
}

fn transfer_tokens_to_pool(program_id: &Pubkey,
                           amount: u64,
                           accounts: &[&AccountInfo]) -> ProgramResult {
    let source_info = accounts[0];
    let signer_info = accounts[1];
    let token_mint_id = crate::config::token_mint::id();
    let token_authority = Pubkey::find_program_address(&[b"Deposit"], program_id).0;
    let pool_pubkey =
        spl_associated_token_account::get_associated_token_address(&token_authority, &token_mint_id);

    let instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_info.key,
        &pool_pubkey,
        signer_info.key,
        &[],
        amount
    )?;

    let account_infos: &[AccountInfo] = &[
        source_info.clone(),
        //pool.clone(),
        signer_info.clone(),
        //token_program.clone(),
    ];

    invoke(&instruction, account_infos)?;
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
const fn delete_token_account() -> ProgramResult {
    Ok(())
}
