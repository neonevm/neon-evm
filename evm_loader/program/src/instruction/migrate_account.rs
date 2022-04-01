use evm::U256;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey
};
use spl_associated_token_account::get_associated_token_address;

use crate::account::{
    ether_account,
    program,
    token,
    AccountData,
    EthereumAccount,
    Operator,
    TAG_EMPTY,
};
use crate::config::token_mint;

type EthereumAccountV1<'a> = AccountData<'a, ether_account::DataV1>;

fn convert_from_v1(v1: EthereumAccountV1, balance: U256) -> Result<EthereumAccount, ProgramError> {
    let null = Pubkey::new_from_array([0_u8; 32]);

    let data = ether_account::Data {
        address: v1.ether,
        bump_seed: v1.nonce,
        trx_count: v1.trx_count,
        balance,
        code_account: if v1.code_account == null { None } else { Some(v1.code_account) },
        rw_blocked: v1.rw_blocked_acc.is_some(),
        ro_blocked_count: v1.ro_blocked_cnt,
    };

    let info = v1.info;
    drop(v1);

    info.data.borrow_mut()[0] = TAG_EMPTY; // reinit
    EthereumAccount::init(info, data)
}

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
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0]) }?,
        ethereum_account: EthereumAccountV1::from_account(program_id, &accounts[1])?,
        token_balance_account: token::State::from_account(&accounts[2])?,
        token_pool_account: token::State::from_account(&accounts[3])?,
        authority_info: &accounts[4],
        token_program: program::Token::from_account(&accounts[5])?,
    };

    validate(program_id, &parsed_accounts)?;
    execute(parsed_accounts)?;

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
fn execute(accounts: Accounts) -> ProgramResult {
    debug_print!("MigrateAccount: execute");
    let amount = accounts.token_balance_account.amount;

    debug_print!("MigrateAccount: convert_from_v1");
    let ethereum_account = convert_from_v1(
        accounts.ethereum_account,
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
