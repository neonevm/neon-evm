use arrayref::array_ref;
use ethnum::U256;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_program::program::invoke_signed;
use spl_associated_token_account::get_associated_token_address;

use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, Operator, program, token};
use crate::types::Address;

struct Accounts<'a> {
    source: token::State<'a>,
    pool: token::State<'a>,
    ethereum_account: &'a AccountInfo<'a>,
    token_program: program::Token<'a>,
    operator: Operator<'a>,
    system_program: program::System<'a>,
}

const AUTHORITY_SEED: &[u8] = b"Deposit";

impl<'a> Accounts<'a> {
    pub fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Accounts<'a>, ProgramError> {
        Ok(Accounts {
            source: token::State::from_account(&accounts[0])?,
            pool: token::State::from_account(&accounts[1])?,
            ethereum_account: &accounts[2],
            token_program: program::Token::from_account(&accounts[3])?,
            operator: unsafe { Operator::from_account_not_whitelisted(&accounts[4]) }?,
            system_program: program::System::from_account(&accounts[5])?,
        })
    }
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Deposit");

    let parsed_accounts = Accounts::from_slice(accounts)?;
    let ethereum_address = Address::from(*array_ref![instruction, 0, 20]);

    let ethereum_bump_seed = validate(program_id, &parsed_accounts, &ethereum_address)?;
    execute(program_id, &parsed_accounts, ethereum_address, ethereum_bump_seed)
}

fn validate(
    program_id: &Pubkey,
    accounts: &Accounts,
    ethereum_address: &Address,
) -> Result<u8, ProgramError> {
    let (expected_solana_address, ethereum_bump_seed) = ethereum_address.find_solana_address(program_id);
    if expected_solana_address != *accounts.ethereum_account.key {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected PDA address {}",
            accounts.ethereum_account.key,
            expected_solana_address
        );
    }

    if accounts.source.mint != crate::config::token_mint::id() {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected Neon Token account",
            accounts.source.info.key
        );
    }

    let (authority_address, _) = Pubkey::find_program_address(&[AUTHORITY_SEED], program_id);
    let expected_pool_address = get_associated_token_address(
        &authority_address,
        &crate::config::token_mint::id(),
    );

    if accounts.pool.info.key != &expected_pool_address {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected Neon Token Pool {}",
            accounts.pool.info.key,
            expected_pool_address
        );
    }

    if accounts.pool.mint != crate::config::token_mint::id() {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected Neon Token account",
            accounts.pool.info.key
        );
    }

    if !accounts.source.delegate.contains(accounts.ethereum_account.key) {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected tokens delegated to an user account",
            accounts.source.info.key
        );
    }

    if accounts.source.delegated_amount < 1 {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} - expected positive tokens amount delegated to an user account",
            accounts.source.info.key
        );
    }

    Ok(ethereum_bump_seed)
}

fn execute<'a>(
    program_id: &'a Pubkey,
    accounts: &Accounts,
    ethereum_address: Address,
    ethereum_bump_seed: u8,
) -> ProgramResult {
    let signers_seeds: &[&[&[u8]]] = &[&[
        &[ACCOUNT_SEED_VERSION],
        ethereum_address.as_bytes(),
        &[ethereum_bump_seed],
    ]];

    let instruction = spl_token::instruction::transfer(
        accounts.token_program.key,
        accounts.source.info.key,
        accounts.pool.info.key,
        accounts.ethereum_account.key,
        &[],
        accounts.source.delegated_amount,
    )?;

    let account_infos: &[AccountInfo] = &[
        accounts.source.info.clone(),
        accounts.pool.info.clone(),
        accounts.ethereum_account.clone(),
        accounts.token_program.clone(),
    ];

    invoke_signed(&instruction, account_infos, signers_seeds)?;

    if solana_program::system_program::check_id(accounts.ethereum_account.owner) {
        EthereumAccount::create_and_init_account(
            &accounts.system_program,
            program_id,
            &accounts.operator,
            ethereum_address,
            accounts.ethereum_account,
            ethereum_bump_seed,
            EthereumAccount::SIZE,
        )?;
    }

    assert!(crate::config::token_mint::decimals() <= 18);
    let additional_decimals: u32 = (18 - crate::config::token_mint::decimals()).into();
    let deposit = U256::from(accounts.source.delegated_amount) * U256::from(10_u64.pow(additional_decimals));
    let mut ethereum_account = EthereumAccount::from_account(program_id, accounts.ethereum_account)?;
    ethereum_account.balance = ethereum_account.balance.checked_add(deposit)
        .ok_or_else(||
            E!(
                ProgramError::InvalidArgument;
                "Account {} - balance overflow",
                ethereum_address
            )
        )?;

    Ok(())
}
