use arrayref::array_ref;
use evm::{H160, U256};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_program::program::invoke_signed;
use solana_program::program_option::COption;
use spl_associated_token_account::get_associated_token_address;

use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, Operator, program, token};
use crate::account::program::EtherAccountParams;

struct Accounts<'a> {
    source: token::State<'a>,
    pool: Option<token::State<'a>>,
    ethereum_account: &'a AccountInfo<'a>,
    token_program: program::Token<'a>,
    operator: Operator<'a>,
    system_program: program::System<'a>,
}

const AUTHORITY_SEED: &[u8] = b"Deposit";

impl<'a> Accounts<'a> {
    pub fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Accounts<'a>, ProgramError> {
        let source = token::State::from_account(&accounts[0])?;
        let pool = if source.delegate.is_some() {
            Some(token::State::from_account(&accounts[1])?)
        } else {
            None
        };
        Ok(Accounts {
            source,
            pool,
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
    let ethereum_address = H160::from(array_ref![instruction, 0, 20]);

    let ethereum_bump_seed = validate(program_id, &parsed_accounts, &ethereum_address)?;
    execute(program_id, &parsed_accounts, ethereum_address, ethereum_bump_seed)
}

fn validate(
    program_id: &Pubkey,
    accounts: &Accounts,
    ethereum_address: &H160,
) -> Result<u8, ProgramError> {
    let program_seeds = [&[ACCOUNT_SEED_VERSION], ethereum_address.as_bytes()];
    let (expected_solana_address, ethereum_bump_seed) =
        Pubkey::find_program_address(&program_seeds, program_id);
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

    if let Some(pool) = &accounts.pool {
        let (authority_address, _) = Pubkey::find_program_address(&[AUTHORITY_SEED], program_id);
        let expected_pool_address = get_associated_token_address(
            &authority_address,
            &crate::config::token_mint::id(),
        );

        if pool.info.key != &expected_pool_address {
            return Err!(
                ProgramError::InvalidArgument;
                "Account {} - expected Neon Token Pool {}",
                pool.info.key,
                expected_pool_address
            );
        }

        if pool.mint != crate::config::token_mint::id() {
            return Err!(
                ProgramError::InvalidArgument;
                "Account {} - expected Neon Token account",
                pool.info.key
            );
        }

        if let COption::Some(delegate) = &accounts.source.delegate {
            if delegate != accounts.ethereum_account.key {
                return Err!(
                    ProgramError::InvalidArgument;
                    "Account {} - expected tokens delegated to an user account",
                    accounts.source.info.key
                );
            }
        }
    }

    Ok(ethereum_bump_seed)
}

fn execute<'a>(
    program_id: &'a Pubkey,
    accounts: &Accounts,
    ethereum_address: H160,
    ethereum_bump_seed: u8,
) -> ProgramResult {
    let amount = accounts.source.delegated_amount;
    let deposit = if accounts.source.delegate.is_none() || amount == 0 {
        U256::zero()
    } else {
        let pool = accounts.pool.as_ref()
            .expect("Pool must be set in a case of delegated amount");

        let signers_seeds: &[&[&[u8]]] = &[&[
            &[ACCOUNT_SEED_VERSION],
            ethereum_address.as_bytes(),
            &[ethereum_bump_seed],
        ]];

        let instruction = spl_token::instruction::transfer(
            accounts.token_program.key,
            accounts.source.info.key,
            pool.info.key,
            accounts.ethereum_account.key,
            &[],
            amount,
        )?;

        let account_infos: &[AccountInfo] = &[
            accounts.source.info.clone(),
            pool.info.clone(),
            accounts.ethereum_account.clone(),
            accounts.token_program.clone(),
        ];

        invoke_signed(&instruction, account_infos, signers_seeds)?;

        assert!(crate::config::token_mint::decimals() <= 18);
        let additional_decimals: u32 = (18 - crate::config::token_mint::decimals()).into();

        U256::from(amount) * U256::from(10_u64.pow(additional_decimals))
    };

    if solana_program::system_program::check_id(accounts.ethereum_account.owner) {
        return accounts.system_program.create_account(
            program_id,
            &accounts.operator,
            &EtherAccountParams {
                address: ethereum_address,
                info: accounts.ethereum_account,
                bump_seed: ethereum_bump_seed,
                space: EthereumAccount::SIZE,
                balance: deposit,
            },
        );
    }

    if amount > 0 {
        let mut ethereum_account = EthereumAccount::from_account(program_id, accounts.ethereum_account)?;
        ethereum_account.balance = ethereum_account.balance.checked_add(deposit)
            .ok_or_else(||
                E!(
                    ProgramError::InvalidArgument;
                    "Account {} - balance overflow",
                    ethereum_address
                )
            )?;
    }

    Ok(())
}
