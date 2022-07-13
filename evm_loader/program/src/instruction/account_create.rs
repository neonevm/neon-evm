use arrayref::{array_ref, array_refs};
use evm::{H160, U256};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::account::{ACCOUNT_SEED_VERSION, ether_contract, EthereumAccount, Operator, program};

struct Accounts<'a> {
    operator: Operator<'a>,
    system_program: program::System<'a>,
    ether_account: &'a AccountInfo<'a>,
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Create Account");

    let parsed_accounts = Accounts {
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0]) }?,
        system_program: program::System::from_account(&accounts[1])?,
        ether_account: &accounts[2],
    };

    let instruction = array_ref![instruction, 0, 20 + 1 + 4];
    let (address, bump_seed, code_size) = array_refs![instruction, 20, 1, 4];

    let address = H160::from(address);
    let bump_seed = u8::from_le_bytes(*bump_seed);
    let code_size = u32::from_le_bytes(*code_size) as usize;

    validate(program_id, &parsed_accounts, &address, bump_seed)?;
    execute(program_id, &parsed_accounts, address, bump_seed, code_size)
}

fn validate(program_id: &Pubkey, accounts: &Accounts, address: &H160, bump_seed: u8) -> ProgramResult {
    if !solana_program::system_program::check_id(accounts.ether_account.owner) {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected system owned", accounts.ether_account.key);
    }

    let program_seeds = [ &[ACCOUNT_SEED_VERSION], address.as_bytes()];
    let (expected_address, expected_bump_seed) = Pubkey::find_program_address(&program_seeds, program_id);
    if expected_address != *accounts.ether_account.key {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected PDA address {}", accounts.ether_account.key, expected_address);
    }
    if expected_bump_seed != bump_seed {
        return Err!(ProgramError::InvalidArgument; "Invalid bump seed, expected = {} found = {}", expected_bump_seed, bump_seed);
    }

    Ok(())
}

fn execute(program_id: &Pubkey, accounts: &Accounts, address: H160, bump_seed: u8, code_size: usize) -> ProgramResult {
    let program_seeds = &[ &[ACCOUNT_SEED_VERSION], address.as_bytes(), &[bump_seed]];
    accounts.system_program.create_pda_account(
        program_id,
        &accounts.operator,
        accounts.ether_account,
        program_seeds,
        EthereumAccount::SIZE + ether_contract::Extension::size_needed(code_size),
    )?;
    
    EthereumAccount::init(
        accounts.ether_account,
        crate::account::ether_account::Data {
            address,
            bump_seed,
            trx_count: 0,
            balance: U256::zero(),
            rw_blocked: false,
            ro_blocked_count: 0,
            generation: 0,
            code_size: 0,
        },
    )?;

    Ok(())
}
