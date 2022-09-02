use evm::U256;
use crate::account::{program, token, EthereumAccount, ACCOUNT_SEED_VERSION};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_program::program::invoke_signed;
use spl_associated_token_account::get_associated_token_address;

struct Accounts<'a> {
    source: token::State<'a>,
    pool: token::State<'a>,
    ethereum_account: EthereumAccount<'a>,
    token_program: program::Token<'a>,
}


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Deposit 2");

    let mut parsed_accounts = Accounts {
        source: token::State::from_account(&accounts[0])?,
        pool: token::State::from_account(&accounts[1])?,
        ethereum_account: EthereumAccount::from_account(program_id, &accounts[2])?,
        token_program: program::Token::from_account(&accounts[3])?,
    };

    validate(program_id, &parsed_accounts)?;
    execute(&mut parsed_accounts)
}

fn validate(program_id: &Pubkey, accounts: &Accounts) -> Result<(), ProgramError> {
    let (authority_address, _) = Pubkey::find_program_address(&[b"Deposit"], program_id);
    let expected_pool_address = get_associated_token_address(&authority_address, &crate::config::token_mint::id());
    
    if accounts.pool.info.key != &expected_pool_address {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected Neon Token Pool {}", accounts.pool.info.key, expected_pool_address);
    }

    if accounts.source.mint != crate::config::token_mint::id() {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected Neon Token account", accounts.source.info.key);
    }

    if accounts.pool.mint != crate::config::token_mint::id() {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected Neon Token account", accounts.pool.info.key);
    }

    if !accounts.source.delegate.contains(accounts.ethereum_account.info.key) {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected tokens delegated to an user account", accounts.source.info.key);
    }

    Ok(())
}

fn execute(accounts: &mut Accounts) -> ProgramResult {
    let amount = accounts.source.delegated_amount;

    {
        let signers_seeds: &[&[&[u8]]] = &[&[
            &[ACCOUNT_SEED_VERSION], 
            accounts.ethereum_account.address.as_bytes(),
            &[accounts.ethereum_account.bump_seed] 
        ]];

        let instruction = spl_token::instruction::transfer(
            accounts.token_program.key,
            accounts.source.info.key,
            accounts.pool.info.key,
            accounts.ethereum_account.info.key,
            &[],
            amount
        )?;

        let account_infos: &[AccountInfo] = &[
            accounts.source.info.clone(),
            accounts.pool.info.clone(),
            accounts.ethereum_account.info.clone(),
            accounts.token_program.clone(),
        ];

        invoke_signed(&instruction, account_infos, signers_seeds)?;
    }


    assert!(crate::config::token_mint::decimals() <= 18);
    let additional_decimals: u32 = (18 - crate::config::token_mint::decimals()).into();

    let deposit = U256::from(amount) * U256::from(10_u64.pow(additional_decimals));
    accounts.ethereum_account.balance = accounts.ethereum_account.balance.checked_add(deposit)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - balance overflow", accounts.ethereum_account.address))?;

    Ok(())
}