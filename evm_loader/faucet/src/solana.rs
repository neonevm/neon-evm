//! Faucet Solana utilities module.

use std::mem;
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use color_eyre::{eyre::eyre, Result};
use tracing::info;

use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer as _;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{system_program, sysvar};

use crate::{config, ethereum};

lazy_static::lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::default());
}

/// Creates the signleton instance of RpcClient.
pub fn init_client(url: String) {
    tokio::task::spawn_blocking(|| CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new(url)));
}

/// Transfers `amount` of tokens.
pub async fn transfer_token(
    signer: Keypair,
    ether_address: ethereum::Address,
    amount: u64,
) -> Result<()> {
    let evm_loader_id = Pubkey::from_str(&config::solana_evm_loader())?;
    let token_mint_id = evm_loader::token::token_mint::id();

    let signer_account = signer.pubkey();
    let signer_token_account =
        spl_associated_token_account::get_associated_token_address(&signer_account, &token_mint_id);

    let (account, _nonce) = make_solana_program_address(&ether_address, &evm_loader_id);
    let token_account =
        spl_associated_token_account::get_associated_token_address(&account, &token_mint_id);

    let r = tokio::task::spawn_blocking(move || -> Result<()> {
        let client = get_client();
        let mut instructions = vec![];

        let balance = client.get_token_account_balance(&token_account);
        let balance_exists = balance.is_ok();
        if balance_exists {
            info!("Token balance of recipient is {:?}", balance.unwrap());
            info!("Ether {:?}", client.get_account(&account)?);
        } else {
            info!("Token balance doesn not exist");
            let ether_account = client.get_account(&account);
            let ether_account_exists = ether_account.is_ok();
            if ether_account_exists {
                info!("Ether {:?}", ether_account.unwrap());
            } else {
                info!("Ether account doesn not exist; will be created");
                instructions.push(create_ether_account_instruction(
                    signer_account,
                    evm_loader_id,
                    ether_address,
                ));
            }
        }

        let token_decimals = evm_loader::token::token_mint::decimals();
        let factor = 10_u64.pow(token_decimals as u32);
        let amount = amount * factor;

        info!("spl_token id = {}", spl_token::id());
        info!("signer_token_account = {}", signer_token_account);
        info!("token_mint_id = {}", token_mint_id);
        info!("token_account = {}", token_account);
        info!("signer_account = {}", signer_account);
        info!("amount = {}", amount);
        info!("token_decimals = {}", token_decimals);
        instructions.push(spl_token::instruction::transfer_checked(
            &spl_token::id(),
            &signer_token_account,
            &token_mint_id,
            &token_account,
            &signer_account,
            &[],
            amount,
            token_decimals,
        )?);

        if instructions.is_empty() {
            return Err(eyre!("No instructions to submit"));
        }

        info!("Creating message...");
        let message = Message::new(&instructions, Some(&signer.pubkey()));
        info!("Creating transaction...");
        let mut tx = Transaction::new_unsigned(message);
        info!("Getting recent blockhash...");
        let (blockhash, _) = client.get_recent_blockhash()?;
        info!("Signing transaction...");
        tx.try_sign(&[&signer], blockhash)?;
        info!("Sending and confirming transaction...");
        client.send_and_confirm_transaction(&tx)?;
        info!("Transaction is confirmed");

        Ok(())
    })
    .await?;

    if let Err(e) = r {
        return Err(eyre!("{:?}", e));
    }

    Ok(())
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
    let token_mint_id = evm_loader::token::token_mint::id();

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
