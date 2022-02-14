use crate::account::{ACCOUNT_SEED_VERSION, Operator, program, token, sysvar, EthereumAccount};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, program_pack::Pack
};

struct Accounts<'a> {
    operator: Operator<'a>,
    user_token: &'a AccountInfo<'a>,
    user: EthereumAccount<'a>,
    erc20_contract: EthereumAccount<'a>,
    mint: token::Mint<'a>,
    system_program: program::System<'a>,
    token_program: program::Token<'a>,
    rent: sysvar::Rent<'a>,
}


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Create ERC20 Wrapper Account");

    let parsed_accounts = Accounts {
        operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0]) }?,
        user_token: &accounts[1],
        user: EthereumAccount::from_account(program_id, &accounts[2])?,
        erc20_contract: EthereumAccount::from_account(program_id, &accounts[3])?,
        mint: token::Mint::from_account(&accounts[4])?,
        system_program: program::System::from_account(&accounts[5])?,
        token_program: program::Token::from_account(&accounts[6])?,
        rent: sysvar::Rent::from_account(&accounts[7])?,
    };

    let bump_seed = validate(program_id, &parsed_accounts)?;
    execute(&parsed_accounts, bump_seed)
}

fn validate(program_id: &Pubkey, accounts: &Accounts) -> Result<u8, ProgramError> {
    if !solana_program::system_program::check_id(accounts.user_token.owner) {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected system owned", accounts.user_token.key);
    }

    if accounts.erc20_contract.code_account.is_none() {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected ERC20 contract", accounts.erc20_contract.address);
    }

    let seeds: &[&[u8]] = &[
        &[ACCOUNT_SEED_VERSION], b"ERC20Balance", &accounts.mint.info.key.to_bytes(),
        accounts.erc20_contract.address.as_bytes(), accounts.user.address.as_bytes()
    ];

    let (expected_address, bump_seed) = Pubkey::find_program_address(seeds, program_id);
    if *accounts.user_token.key != expected_address {
        return Err!(ProgramError::InvalidArgument; "Account {} - expected PDA address {}", accounts.user_token.key, expected_address);
    }

    Ok(bump_seed)
}

fn execute(accounts: &Accounts, bump_seed: u8) -> ProgramResult {
    let seeds: &[&[u8]] = &[
        &[ACCOUNT_SEED_VERSION], b"ERC20Balance", &accounts.mint.info.key.to_bytes(),
        accounts.erc20_contract.address.as_bytes(), accounts.user.address.as_bytes(),
        &[bump_seed]
    ];

    accounts.system_program.create_pda_account(
        &spl_token::id(),
        &accounts.operator,
        accounts.user_token,
        seeds,
        spl_token::state::Account::LEN,
    )?;

    accounts.token_program.initialize_account(
        accounts.user_token,
        &accounts.mint,
        &accounts.user,
        &accounts.rent
    )
}
