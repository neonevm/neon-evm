use crate::account::{EthereumAccount, EthereumContract, Operator};
use crate::account;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;


struct Accounts<'a> {
    ethereum_account: EthereumAccount<'a>,
    code_account: &'a AccountInfo<'a>, // or Pubkey::default()
    new_code_account: &'a AccountInfo<'a>,
    operator: Operator<'a>,
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Resize Contract Account");

    let parsed_accounts = Accounts {
        ethereum_account: EthereumAccount::from_account(program_id, &accounts[0])?,
        code_account: &accounts[1],
        new_code_account: &accounts[2],
        operator: Operator::from_account(&accounts[3])?,
    };

    let seed = std::str::from_utf8(instruction)
        .map_err(|e| E!(ProgramError::InvalidInstructionData; "Seed decode error={:?}", e))?;

    validate(program_id, &parsed_accounts, seed)?;
    execute(parsed_accounts)
}

fn validate(program_id: &Pubkey, accounts: &Accounts, seed: &str) -> ProgramResult {
    let old_code_account = accounts.code_account;
    let new_code_account = accounts.new_code_account;
    let account = &accounts.ethereum_account;
    let operator = &accounts.operator;

    if old_code_account.data_len() >= new_code_account.data_len() {
        return Err!(ProgramError::InvalidAccountData; "New code account size is less than or equal to current code account size");
    }

    if account.rw_blocked || account.ro_blocked_count > 0 {
        return Err!(ProgramError::InvalidInstructionData; "Account {} - is blocked", account.address);
    }

    let code_account_key = account.code_account.unwrap_or_default();
    if code_account_key != *old_code_account.key {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected key {}", old_code_account.key, code_account_key);
    }
    if (code_account_key == Pubkey::default()) && (account.trx_count != 0) {
        return Err!(ProgramError::InvalidArgument; "Cannot change user account to contract account");
    }

    let expected_address = Pubkey::create_with_seed(operator.key, seed, program_id)?;
    if *new_code_account.key != expected_address {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected key {}", new_code_account.key, expected_address);
    }

    if new_code_account.owner != program_id {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected program owned", new_code_account.key);
    }

    let tag = crate::account::tag(program_id, new_code_account)?;
    if tag != crate::account::TAG_EMPTY {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected tag empty", new_code_account.key);
    }

    let rent = Rent::get()?;
    if !rent.is_exempt(new_code_account.lamports(), new_code_account.data_len()) {
        return Err!(ProgramError::InvalidArgument; "Account {} - is not rent exempt", new_code_account.key);
    }

    Ok(())
}

fn execute(accounts: Accounts) -> ProgramResult {
    let Accounts {
        mut ethereum_account,
        code_account,
        new_code_account,
        operator,
    } = accounts;

    if *code_account.key == Pubkey::default() {
        EthereumContract::init(new_code_account, account::ether_contract::Data {
            owner: *ethereum_account.info.key,
            code_size: 0_u32,
            generation: 0_u32,
        })?;
    } else {
        {
            let source = code_account.try_borrow_mut_data()?;
            let mut dest = new_code_account.try_borrow_mut_data()?;

            dest[..source.len()].copy_from_slice(&source);
            dest[source.len()..].fill(0);
        }

        unsafe { crate::account::delete(code_account, &operator) }?;
    }

    ethereum_account.code_account = Some(*new_code_account.key);

    Ok(())
}
