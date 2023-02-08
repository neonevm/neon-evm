use arrayref::array_ref;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::account::{program, EthereumAccount, Operator};
use crate::types::Address;

struct Accounts<'a> {
    operator: Operator<'a>,
    system_program: program::System<'a>,
    ether_account: &'a AccountInfo<'a>,
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> ProgramResult {
    solana_program::msg!("Instruction: Create Account");

    let parsed_accounts = Accounts {
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0]) }?,
        system_program: program::System::from_account(&accounts[1])?,
        ether_account: &accounts[2],
    };

    let address = array_ref![instruction, 0, 20];
    let address = Address::from(*address);
    solana_program::msg!("Address: {}", address);

    let bump_seed = validate(program_id, &parsed_accounts, &address)?;
    execute(program_id, &parsed_accounts, address, bump_seed)
}

fn validate(
    program_id: &Pubkey,
    accounts: &Accounts,
    address: &Address,
) -> Result<u8, ProgramError> {
    if !solana_program::system_program::check_id(accounts.ether_account.owner) {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected system owned", accounts.ether_account.key);
    }

    let (expected_address, bump_seed) = address.find_solana_address(program_id);
    if expected_address != *accounts.ether_account.key {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected PDA address {}", accounts.ether_account.key, expected_address);
    }

    Ok(bump_seed)
}

fn execute(
    program_id: &Pubkey,
    accounts: &Accounts,
    address: Address,
    bump_seed: u8,
) -> ProgramResult {
    EthereumAccount::create_and_init_account(
        &accounts.system_program,
        program_id,
        &accounts.operator,
        address,
        accounts.ether_account,
        bump_seed,
        EthereumAccount::SIZE,
    )?;

    Ok(())
}
