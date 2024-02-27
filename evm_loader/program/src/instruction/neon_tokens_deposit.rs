use arrayref::array_ref;
use ethnum::U256;
use solana_program::program::invoke_signed;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey, rent::Rent, sysvar::Sysvar};
use spl_associated_token_account::get_associated_token_address;

use crate::account::{program, token, AccountsDB, BalanceAccount, Operator, ACCOUNT_SEED_VERSION};
use crate::config::{CHAIN_ID_LIST, DEFAULT_CHAIN_ID};
use crate::error::{Error, Result};
use crate::types::Address;

struct Accounts<'a> {
    mint: token::Mint<'a>,
    source: token::State<'a>,
    pool: token::State<'a>,
    balance_account: &'a AccountInfo<'a>,
    contract_account: &'a AccountInfo<'a>,
    token_program: program::Token<'a>,
    operator: Operator<'a>,
    system_program: program::System<'a>,
}

const AUTHORITY_SEED: &[u8] = b"Deposit";

impl<'a> Accounts<'a> {
    pub fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Accounts<'a>> {
        Ok(Accounts {
            mint: token::Mint::from_account(&accounts[0])?,
            source: token::State::from_account(&accounts[1])?,
            pool: token::State::from_account(&accounts[2])?,
            balance_account: &accounts[3],
            contract_account: &accounts[4],
            token_program: program::Token::from_account(&accounts[5])?,
            operator: unsafe { Operator::from_account_not_whitelisted(&accounts[6]) }?,
            system_program: program::System::from_account(&accounts[7])?,
        })
    }
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Deposit");

    let parsed_accounts = Accounts::from_slice(accounts)?;

    let address = array_ref![instruction, 0, 20];
    let address = Address::from(*address);

    let chain_id = array_ref![instruction, 20, 8];
    let chain_id = u64::from_le_bytes(*chain_id);

    validate(program_id, &parsed_accounts, address, chain_id)?;
    execute(program_id, parsed_accounts, address, chain_id)
}

fn validate(
    program_id: &Pubkey,
    accounts: &Accounts,
    address: Address,
    chain_id: u64,
) -> Result<()> {
    let balance_account = *accounts.balance_account.key;
    let contract_account = *accounts.contract_account.key;
    let pool = *accounts.pool.info.key;
    let mint = *accounts.mint.info.key;

    let (expected_pubkey, _) = address.find_balance_address(program_id, chain_id);
    if expected_pubkey != balance_account {
        return Err(Error::AccountInvalidKey(balance_account, expected_pubkey));
    }

    let (expected_pubkey, _) = address.find_solana_address(program_id);
    if expected_pubkey != contract_account {
        return Err(Error::AccountInvalidKey(contract_account, expected_pubkey));
    }

    let Ok(chain_id_index) = CHAIN_ID_LIST.binary_search_by_key(&chain_id, |c| c.0) else {
        return Err(Error::InvalidChainId(chain_id));
    };

    let expected_mint = CHAIN_ID_LIST[chain_id_index].2;
    if mint != expected_mint {
        return Err(Error::AccountInvalidKey(mint, expected_mint));
    }

    let (authority_address, _) = Pubkey::find_program_address(&[AUTHORITY_SEED], program_id);
    let expected_pool = get_associated_token_address(&authority_address, &mint);
    if pool != expected_pool {
        return Err(Error::AccountInvalidKey(pool, expected_pool));
    }

    if (accounts.pool.mint != mint) || (accounts.source.mint != mint) {
        return Err(Error::from("Invalid token mint"));
    }

    let is_correct_delegate = accounts
        .source
        .delegate
        .contains(accounts.balance_account.key);

    if !is_correct_delegate {
        return Err(Error::from("Expected tokens delegated to balance account"));
    }

    if accounts.source.delegated_amount < 1 {
        return Err(Error::from("Expected positive tokens amount delegated"));
    }

    Ok(())
}

fn execute(program_id: &Pubkey, accounts: Accounts, address: Address, chain_id: u64) -> Result<()> {
    let (_, bump_seed) = address.find_balance_address(program_id, chain_id);
    let signer_seeds: &[&[u8]] = &[
        &[ACCOUNT_SEED_VERSION],
        address.as_bytes(),
        &U256::from(chain_id).to_be_bytes(),
        &[bump_seed],
    ];

    let instruction = spl_token::instruction::transfer(
        accounts.token_program.key,
        accounts.source.info.key,
        accounts.pool.info.key,
        accounts.balance_account.key,
        &[],
        accounts.source.delegated_amount,
    )?;

    let account_infos: &[AccountInfo] = &[
        accounts.source.info.clone(),
        accounts.pool.info.clone(),
        accounts.balance_account.clone(),
        accounts.token_program.clone(),
    ];

    invoke_signed(&instruction, account_infos, &[signer_seeds])?;

    let token_decimals = accounts.mint.decimals;
    assert!(token_decimals <= 18);

    let additional_decimals: u32 = (18 - token_decimals).into();
    let deposit = U256::from(accounts.source.delegated_amount) * 10_u128.pow(additional_decimals);

    let accounts_db = AccountsDB::new(
        &[
            accounts.balance_account.clone(),
            accounts.contract_account.clone(),
        ],
        accounts.operator,
        None,
        Some(accounts.system_program),
        None,
    );

    let mut excessive_lamports = 0;
    if chain_id == DEFAULT_CHAIN_ID {
        // we don't have enough accounts to update non Neon chains
        excessive_lamports += crate::account::legacy::update_legacy_accounts(&accounts_db)?;
    }

    let rent = Rent::get()?;

    let mut balance_account = BalanceAccount::create(address, chain_id, &accounts_db, None, &rent)?;
    balance_account.mint(deposit)?;

    **accounts_db.operator().try_borrow_mut_lamports()? += excessive_lamports;

    Ok(())
}
