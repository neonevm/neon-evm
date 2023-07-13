use crate::{
    account::EthereumAccount,
    error::{Error, Result},
};
use arrayref::array_ref;
use solana_program::{
    account_info::AccountInfo,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    program_error::ProgramError,
    pubkey::Pubkey,
};

struct Accounts<'a> {
    program_data: &'a AccountInfo<'a>,
    signer: &'a AccountInfo<'a>,
    ethereum_account: EthereumAccount<'a>,
}

fn get_program_upgrade_authority(
    program_id: &Pubkey,
    program_data: &AccountInfo,
) -> Result<Pubkey> {
    let (expected_key, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());

    if *program_data.key != expected_key {
        return Err(Error::AccountInvalidKey(*program_data.key, expected_key));
    }

    match bincode::deserialize(&program_data.data.borrow())? {
        UpgradeableLoaderState::ProgramData {
            upgrade_authority_address,
            ..
        } => upgrade_authority_address
            .ok_or_else(|| "NeonEVM must have valid upgrade authority".into()),
        _ => Err(Error::AccountInvalidData(*program_data.key)),
    }
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction: &[u8],
) -> Result<()> {
    solana_program::msg!("Instruction: TEST Set Account Nonce");

    if cfg!(feature = "mainnet") {
        return Err(ProgramError::InvalidInstructionData.into());
    }

    if !(cfg!(feature = "devnet") || cfg!(feature = "testnet") || cfg!(feature = "ci")) {
        return Err(ProgramError::InvalidInstructionData.into());
    }

    let nonce = u64::from_le_bytes(*array_ref![instruction, 0, 8]);

    let mut accounts = Accounts {
        signer: &accounts[0],
        program_data: &accounts[1],
        ethereum_account: EthereumAccount::from_account(program_id, &accounts[2])?,
    };

    if !accounts.signer.is_signer {
        return Err(Error::AccountNotSigner(*accounts.signer.key));
    }

    let upgrade_authority = get_program_upgrade_authority(program_id, accounts.program_data)?;
    if upgrade_authority != *accounts.signer.key {
        return Err("Signer is not program upgrade authority".into());
    }

    accounts.ethereum_account.trx_count = nonce;

    Ok(())
}
