use crate::{
    account::{MainTreasury, program::System, program::Token, Operator},
    config::TREASURY_POOL_SEED,
};
use solana_program::{
    msg,
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_pack::Pack,
    program_error::ProgramError,
    system_program,
    bpf_loader_upgradeable::{
        self,
        UpgradeableLoaderState,
    }
};

struct Accounts<'a> {
    main_treasury: &'a AccountInfo<'a>,
    _program_data: &'a AccountInfo<'a>,
    program_upgrade_auth: &'a AccountInfo<'a>,
    token_program: Token<'a>,
    system_program: System<'a>,
    mint: &'a AccountInfo<'a>,
    payer: Operator<'a>,
}

impl<'a> Accounts<'a> {
    pub fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Accounts<'a>, ProgramError> {
        Ok(Accounts {
            main_treasury: &accounts[0],
            _program_data: &accounts[1],
            program_upgrade_auth: &accounts[2],
            token_program: Token::from_account(&accounts[3])?,
            system_program: System::from_account(&accounts[4])?,
            mint: &accounts[5],
            payer: unsafe { Operator::from_account_not_whitelisted(&accounts[6])}?,
        })
    }
}

fn get_program_upgrade_authority<'a>(program_id: &'a Pubkey, program_data: &'a AccountInfo<'a>) -> Result<Pubkey, ProgramError> {
    let (expected_program_data_key, _) = Pubkey::find_program_address(
        &[program_id.as_ref()], 
        &bpf_loader_upgradeable::id()
    );

    if *program_data.key != expected_program_data_key {
        return Err!(ProgramError::InvalidArgument; "Account {} - invalid current program data account", program_data.key);
    }

    let unpacked_program_data: UpgradeableLoaderState =
        bincode::deserialize(&program_data.data.borrow()).map_err(
            |_| E!(ProgramError::InvalidAccountData; "Unable to deserialize program data")
        )?;
    
    let upgrade_authority: Pubkey = 
        match unpacked_program_data {
            UpgradeableLoaderState::ProgramData { slot: _, upgrade_authority_address } => 
                upgrade_authority_address.ok_or_else(|| E!(ProgramError::InvalidAccountData; "Not upgradeable program" ))?,
            _ => 
                return Err!(ProgramError::InvalidAccountData; "Not ProgramData"),
        };
    
    Ok(upgrade_authority)
}

pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], _instruction: &[u8]) -> ProgramResult {
    msg!("Instruction: Create Main Treasury");

    let accounts = Accounts::from_slice(accounts)?;
    let (expected_key, bump_seed) = MainTreasury::address(program_id);

    if *accounts.main_treasury.key != expected_key {
        return Err!(ProgramError::InvalidArgument; "Account {} - invalid main treasure account", accounts.main_treasury.key);
    }

    if *accounts.mint.key != spl_token::native_mint::id() {
        return Err!(ProgramError::InvalidArgument; "Account {} - not wrapped SOL mint", accounts.mint.key);
    }

    if *accounts.system_program.key != system_program::id() {
        return Err!(ProgramError::InvalidArgument; "Account {} - not system program", accounts.system_program.key);
    }

    if *accounts.token_program.key != spl_token::id() {
        return Err!(ProgramError::InvalidArgument; "Account {} - not spl-token program", accounts.token_program.key);
    }

    let expected_upgrade_auth_key = get_program_upgrade_authority(program_id, accounts._program_data)?;
    if *accounts.program_upgrade_auth.key != expected_upgrade_auth_key {
        return Err!(ProgramError::InvalidArgument; "Account {} - invalid program upgrade authority", accounts.program_upgrade_auth.key);
    }
    if !accounts.program_upgrade_auth.is_signer {
        return Err!(ProgramError::MissingRequiredSignature; "Required signature from program upgrade authority");
    }

    accounts.system_program.create_pda_account(
        &spl_token::id(),
        &accounts.payer,
        accounts.main_treasury,
        &[&TREASURY_POOL_SEED.as_bytes(), &[bump_seed]],
        spl_token::state::Account::LEN,
    )?;

    accounts.token_program.create_account(
        accounts.main_treasury,
        accounts.mint,
        &accounts.program_upgrade_auth,
    )?;

    Ok(())
}