use crate::{
    account::{program::System, program::Token, MainTreasury, Operator},
    config::TREASURY_POOL_SEED,
    error::{Error, Result},
};
use solana_program::{
    account_info::AccountInfo,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
    sysvar::Sysvar,
};

struct Accounts<'a> {
    main_treasury: &'a AccountInfo<'a>,
    program_data: &'a AccountInfo<'a>,
    program_upgrade_auth: &'a AccountInfo<'a>,
    token_program: Token<'a>,
    system_program: System<'a>,
    mint: &'a AccountInfo<'a>,
    payer: Operator<'a>,
}

impl<'a> Accounts<'a> {
    pub fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Accounts<'a>> {
        Ok(Accounts {
            main_treasury: &accounts[0],
            program_data: &accounts[1],
            program_upgrade_auth: &accounts[2],
            token_program: Token::from_account(&accounts[3])?,
            system_program: System::from_account(&accounts[4])?,
            mint: &accounts[5],
            payer: unsafe { Operator::from_account_not_whitelisted(&accounts[6]) }?,
        })
    }
}

fn get_program_upgrade_authority<'a>(
    program_id: &'a Pubkey,
    program_data: &'a AccountInfo<'a>,
) -> Result<Pubkey> {
    let (expected_program_data_key, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());

    if *program_data.key != expected_program_data_key {
        return Err(Error::AccountInvalidKey(
            *program_data.key,
            expected_program_data_key,
        ));
    }

    let unpacked_program_data: UpgradeableLoaderState =
        bincode::deserialize(&program_data.data.borrow())?;

    let upgrade_authority: Pubkey = match unpacked_program_data {
        UpgradeableLoaderState::ProgramData {
            slot: _,
            upgrade_authority_address,
        } => upgrade_authority_address.ok_or_else(|| Error::from("Not upgradeable program"))?,
        _ => return Err(Error::from("Not ProgramData")),
    };

    Ok(upgrade_authority)
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    log_msg!("Instruction: Create Main Treasury");

    let accounts = Accounts::from_slice(accounts)?;
    let (expected_key, bump_seed) = MainTreasury::address(program_id);

    if *accounts.main_treasury.key != expected_key {
        return Err(Error::AccountInvalidKey(
            *accounts.main_treasury.key,
            expected_key,
        ));
    }

    if *accounts.mint.key != spl_token::native_mint::id() {
        return Err(Error::Custom(std::format!(
            "Account {} - not wrapped SOL mint",
            accounts.mint.key
        )));
    }

    if *accounts.system_program.key != system_program::id() {
        return Err(Error::AccountInvalidKey(
            *accounts.system_program.key,
            system_program::id(),
        ));
    }

    if *accounts.token_program.key != spl_token::id() {
        return Err(Error::AccountInvalidKey(
            *accounts.token_program.key,
            spl_token::id(),
        ));
    }

    let expected_upgrade_auth_key =
        get_program_upgrade_authority(program_id, accounts.program_data)?;
    if *accounts.program_upgrade_auth.key != expected_upgrade_auth_key {
        return Err(Error::AccountInvalidKey(
            *accounts.program_upgrade_auth.key,
            expected_upgrade_auth_key,
        ));
    }
    if !accounts.program_upgrade_auth.is_signer {
        return Err(Error::AccountNotSigner(*accounts.program_upgrade_auth.key));
    }

    accounts.system_program.create_pda_account(
        &spl_token::id(),
        &accounts.payer,
        accounts.main_treasury,
        &[TREASURY_POOL_SEED.as_bytes(), &[bump_seed]],
        spl_token::state::Account::LEN,
        &Rent::get()?,
    )?;

    accounts.token_program.create_account(
        accounts.main_treasury,
        accounts.mint,
        accounts.program_upgrade_auth,
    )?;

    Ok(())
}
