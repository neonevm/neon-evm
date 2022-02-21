use evm::U256;

use crate::account::{program, token, EthereumAccount};
use crate::config::token_mint;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg
};

use solana_program::program::invoke_signed;
use spl_associated_token_account::get_associated_token_address;

struct Accounts<'a> {
    source: token::State<'a>,
    pool: token::State<'a>,
    ethereum_account: EthereumAccount<'a>,
    authority: &'a AccountInfo<'a>,
    token_program: program::Token<'a>,
}


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    msg!("Instruction: Deposit");

    let mut parsed_accounts = Accounts {
        source: token::State::from_account(&accounts[0])?,
        pool: token::State::from_account(&accounts[1])?,
        ethereum_account: EthereumAccount::from_account(program_id, &accounts[2])?,
        authority: &accounts[3],
        token_program: program::Token::from_account(&accounts[4])?,
    };

    let bump_seed = validate(program_id, &parsed_accounts)?;
    execute(&mut parsed_accounts, bump_seed)
}

fn validate(program_id: &Pubkey, accounts: &Accounts) -> Result<u8, ProgramError> {
    let (expected_address, bump_seed) = Pubkey::find_program_address(&[b"Deposit"], program_id);
    if accounts.authority.key != &expected_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected PDA address {}",
            accounts.authority.key, expected_address);
    }

    /* Need this? get_associated_token_address is a costly function */
    let expected_pool_address = get_associated_token_address(
        accounts.authority.key,
        &token_mint::id()
    );
    if accounts.pool.info.key != &expected_pool_address {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected Neon Token Pool {}",
            accounts.pool.info.key, expected_pool_address);
    }

    if !accounts.source.delegate.contains(accounts.authority.key) {
        return Err!(ProgramError::InvalidArgument;
            "Account {} - expected tokens delegated to authority account",
            accounts.source.info.key);
    }

    Ok(bump_seed)
}

fn execute(accounts: &mut Accounts, bump_seed: u8) -> ProgramResult {
    let amount = accounts.source.delegated_amount;

    {
        let signers_seeds: &[&[&[u8]]] = &[&[b"Deposit", &[bump_seed]]];

        let instruction = spl_token::instruction::transfer(
            accounts.token_program.key,
            accounts.source.info.key,
            accounts.pool.info.key,
            accounts.authority.key,
            &[],
            amount
        )?;

        let account_infos: &[AccountInfo] = &[
            accounts.source.info.clone(),
            accounts.pool.info.clone(),
            accounts.authority.clone(),
            accounts.token_program.clone(),
        ];

        invoke_signed(&instruction, account_infos, signers_seeds)?;
    }


    assert!(token_mint::decimals() <= 18);
    let additional_decimals: u32 = (18 - token_mint::decimals()).into();

    let deposit = U256::from(amount) * U256::from(10_u64.pow(additional_decimals));
    accounts.ethereum_account.balance = accounts.ethereum_account.balance.checked_add(deposit)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - balance overflow", accounts.ethereum_account.address))?;

    Ok(())
}
