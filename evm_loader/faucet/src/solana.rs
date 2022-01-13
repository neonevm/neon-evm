//! Faucet Solana utilities module.

use std::mem;
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use eyre::{eyre, Result, WrapErr};
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer as _;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{system_program, sysvar};
use tokio::task;
use tracing::info;

use crate::{config, ethereum};

lazy_static::lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::default());
}

/// Creates the signleton instance of RpcClient.
pub fn init_client(url: String) {
    task::spawn_blocking(|| CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new(url)));
}

/// Converts amount of tokens from whole value to fractions (usually 10E-9).
pub fn convert_whole_to_fractions(amount: u64) -> Result<u64> {
    let decimals = config::solana_token_mint_decimals();
    let factor = 10_u64
        .checked_pow(decimals as u32)
        .ok_or_else(|| eyre!("Overflow 10^{}", decimals))?;
    amount
        .checked_mul(factor as u64)
        .ok_or_else(|| eyre!("Overflow {}*{}", amount, factor))
}

/// Transfers `amount` of tokens.
/// When in_fractions == false, amount is treated as whole token amount.
/// When in_fractions == true, amount is treated as amount in galans (10E-9).
pub async fn transfer_token(
    id: &str,
    signer: Vec<u8>,
    ether_address: ethereum::Address,
    amount: u64,
    in_fractions: bool,
) -> Result<()> {
    let evm_loader_id = Pubkey::from_str(&config::solana_evm_loader()).wrap_err_with(|| {
        format!(
            "config::solana_evm_loader returns {}",
            &config::solana_evm_loader()
        )
    })?;
    let token_mint_id = Pubkey::from_str(&config::solana_token_mint_id()).wrap_err_with(|| {
        format!(
            "config::solana_token_mint_id returns {}",
            &config::solana_token_mint_id(),
        )
    })?;

    let signer_account = Keypair::from_bytes(&signer)?.pubkey();
    let signer_token_account =
        spl_associated_token_account::get_associated_token_address(&signer_account, &token_mint_id);

    let (account, _nonce) = make_solana_program_address(&ether_address, &evm_loader_id);
    let token_account =
        spl_associated_token_account::get_associated_token_address(&account, &token_mint_id);

    info!("{} spl_token id = {}", id, spl_token::id());
    info!("{} signer_token_account = {}", id, signer_token_account);
    info!("{} token_mint_id = {}", id, token_mint_id);
    info!("{} token_account = {}", id, token_account);
    info!("{} signer_account = {}", id, signer_account);
    info!("{} amount = {}", id, amount);
    info!(
        "{} token_decimals = {}",
        id,
        config::solana_token_mint_decimals()
    );

    let exists = ether_account_exists(id.to_owned(), account, token_account).await?;
    if !exists {
        if let Err(e) =
            create_ether_account(signer.clone(), signer_account, evm_loader_id, ether_address).await
        {
            return Err(eyre!("{:?}", e));
        }
    }

    if let Err(e) = do_transfer_token(
        id.to_owned(),
        signer,
        signer_token_account,
        token_mint_id,
        token_account,
        signer_account,
        amount,
        in_fractions,
    )
    .await
    {
        return Err(eyre!("{:?}", e));
    }

    Ok(())
}

/// Checks if ether account for Solana account exists.
async fn ether_account_exists(id: String, account: Pubkey, token_account: Pubkey) -> Result<bool> {
    task::spawn_blocking(move || -> Result<bool> {
        let client = get_client();
        let balance = client.get_token_account_balance(&token_account);
        let balance_exists = balance.is_ok();
        if balance_exists {
            info!(
                "{} Token balance of recipient is {:?}",
                id,
                balance.unwrap()
            );
            info!("{} Ether {:?}", id, client.get_account(&account)?);
            Ok(true)
        } else {
            info!("{} Empty balance of token account '{}'", id, token_account);
            let ether_account = client.get_account(&account);
            let ether_account_exists = ether_account.is_ok();
            if ether_account_exists {
                info!("{} Ether {:?}", id, ether_account.unwrap());
                Ok(true)
            } else {
                info!("{} No ether account; will be created", id);
                Ok(false)
            }
        }
    })
    .await?
}

/// Creates ether account. Ignores possible error if the account already created.
async fn create_ether_account(
    signer: Vec<u8>,
    signer_account: Pubkey,
    evm_loader_id: Pubkey,
    ether_address: ethereum::Address,
) -> Result<()> {
    task::spawn_blocking(move || -> Result<()> {
        let client = get_client();

        let mut instructions = Vec::with_capacity(1);
        instructions.push(create_ether_account_instruction(
            signer_account,
            evm_loader_id,
            ether_address,
        ));

        let signer = Keypair::from_bytes(&signer)?;
        let message = Message::new(&instructions, Some(&signer.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        let (blockhash, _) = client.get_recent_blockhash()?;
        tx.try_sign(&[&signer], blockhash)?;
        let _ignore_error = client.send_and_confirm_transaction(&tx);
        Ok(())
    })
    .await?
}

/// Performs transfer from treasure to a Solana account.
async fn do_transfer_token(
    id: String,
    signer: Vec<u8>,
    signer_token_account: Pubkey,
    token_mint_id: Pubkey,
    token_account: Pubkey,
    signer_account: Pubkey,
    amount: u64,
    in_fractions: bool,
) -> Result<()> {
    task::spawn_blocking(move || -> Result<()> {
        let client = get_client();

        let amount = if in_fractions {
            amount
        } else {
            convert_whole_to_fractions(amount)?
        };

        let mut instructions = Vec::with_capacity(1);
        instructions.push(spl_token::instruction::transfer_checked(
            &spl_token::id(),
            &signer_token_account,
            &token_mint_id,
            &token_account,
            &signer_account,
            &[],
            amount,
            config::solana_token_mint_decimals(),
        )?);

        let signer = Keypair::from_bytes(&signer)?;
        let message = Message::new(&instructions, Some(&signer.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        let (blockhash, _) = client.get_recent_blockhash()?;
        tx.try_sign(&[&signer], blockhash)?;
        info!("{} Sending transfer transaction...", id);
        client.send_and_confirm_transaction(&tx)?;
        info!("{} Transfer transaction is confirmed", id);

        Ok(())
    })
    .await?
}

/// Maps an Ethereum address into a Solana address.
/// Copied here from evm_loader/cli/src/account_storage.rs.
fn make_solana_program_address(
    ether_address: &ethereum::Address,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &[evm_loader::account_data::ACCOUNT_SEED_VERSION],
            ether_address.as_bytes(),
        ],
        program_id,
    )
}

/// Returns instruction for creation of account.
fn create_ether_account_instruction(
    signer: Pubkey,
    evm_loader_id: Pubkey,
    ether_address: ethereum::Address,
) -> Instruction {
    let token_mint_id =
        Pubkey::from_str(&config::solana_token_mint_id()).expect("invalid token mint id");

    let (solana_address, nonce) = make_solana_program_address(&ether_address, &evm_loader_id);
    let token_address =
        spl_associated_token_account::get_associated_token_address(&solana_address, &token_mint_id);

    let lamports = 0;
    let space = 0;
    Instruction::new_with_bincode(
        evm_loader_id,
        &evm_loader::instruction::EvmInstruction::CreateAccount {
            lamports,
            space,
            ether: unsafe { mem::transmute(ether_address) },
            nonce,
        },
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(solana_address, false),
            AccountMeta::new(token_address, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(token_mint_id, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
    )
}

struct Client(Arc<RpcClient>);

impl Default for Client {
    fn default() -> Client {
        Client(Arc::new(RpcClient::new(String::default())))
    }
}

/// Returns instance of RpcClient.
fn get_client() -> Arc<RpcClient> {
    CLIENT.lock().unwrap().0.clone()
}
