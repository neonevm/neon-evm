//! Faucet Solana utilities module.

use std::mem;
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use eyre::{eyre, Result, WrapErr};
use tracing::info;

use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
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
    tokio::task::spawn_blocking(|| {
        CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new_with_commitment(
            url,
            CommitmentConfig::confirmed(),
        ))
    });
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

/// Deposits `amount` of tokens from main account to associated account.
/// When `in_fractions` == false, amount is treated as whole token amount.
/// When `in_fractions` == true, amount is treated as amount in galans (10E-9).
pub async fn deposit_token(
    id: &str,
    signer: Keypair,
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

    let signer_pubkey = signer.pubkey();
    let signer_token_pubkey =
        spl_associated_token_account::get_associated_token_address(&signer_pubkey, &token_mint_id);

    let evm_token_pubkey =
        spl_associated_token_account::get_associated_token_address(&evm_loader_id, &token_mint_id);

    let (ether_pubkey, _) = make_solana_program_address(&ether_address, &evm_loader_id);

    let id = id.to_owned();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let client = get_client();
        let mut instructions = Vec::with_capacity(3);

        let ether_account = client.get_account(&ether_pubkey);
        if !ether_account.is_ok() {
            info!(
                "{} No ether account for {}; will be created",
                id, ether_address
            );
            instructions.push(create_ether_account_instruction(
                signer_pubkey,
                evm_loader_id,
                ether_address,
            ));
        }

        let amount = if in_fractions {
            amount
        } else {
            convert_whole_to_fractions(amount)?
        };

        info!("{} spl_token id = {}", id, spl_token::id());
        info!("{} signer_token_pubkey = {}", id, signer_token_pubkey);
        info!("{} token_mint_id = {}", id, token_mint_id);
        info!("{} evm_token_pubkey = {}", id, evm_token_pubkey);
        info!("{} signer_pubkey = {}", id, signer_pubkey);
        info!("{} amount = {}", id, amount);
        info!(
            "{} token_decimals = {}",
            id,
            config::solana_token_mint_decimals()
        );

        instructions.push(spl_approve_instruction(
            spl_token::id(),
            signer_token_pubkey,
            evm_loader_id,
            signer_pubkey,
            amount,
        ));

        instructions.push(deposit_instruction(
            signer_token_pubkey,
            evm_token_pubkey,
            ether_pubkey,
            evm_loader_id,
        ));

        if instructions.is_empty() {
            return Err(eyre!("No instructions to submit"));
        }

        info!("{} Creating message...", id);
        let message = Message::new(&instructions, Some(&signer.pubkey()));
        info!("{} Creating transaction...", id);
        let mut tx = Transaction::new_unsigned(message);
        info!("{} Getting recent blockhash...", id);
        let (blockhash, _) = client.get_recent_blockhash()?;
        info!("{} Signing transaction...", id);
        tx.try_sign(&[&signer], blockhash)?;
        info!("{} Sending and confirming transaction...", id);
        client.send_and_confirm_transaction(&tx)?;
        info!("{} Transaction is confirmed", id);

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
    signer_pubkey: Pubkey,
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
            AccountMeta::new(signer_pubkey, true),
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

/// Returns instruction to approve transfer of NEON tokens.
fn spl_approve_instruction(
    token_program_id: Pubkey,
    source_pubkey: Pubkey,
    delegate_pubkey: Pubkey,
    owner_pubkey: Pubkey,
    amount: u64,
) -> Instruction {
    use spl_token::instruction::TokenInstruction;

    let accounts = vec![
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new_readonly(delegate_pubkey, false),
        AccountMeta::new_readonly(owner_pubkey, true),
    ];

    let data = TokenInstruction::Approve { amount }.pack();

    Instruction {
        program_id: token_program_id,
        accounts,
        data,
    }
}

/// Returns instruction to deposit NEON tokens.
fn deposit_instruction(
    source_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    ether_account_pubkey: Pubkey,
    evm_loader_id: Pubkey,
) -> Instruction {
    Instruction::new_with_bincode(
        evm_loader_id,
        &evm_loader::instruction::EvmInstruction::Deposit,
        vec![
            AccountMeta::new(source_pubkey, false),
            AccountMeta::new(destination_pubkey, false),
            AccountMeta::new(ether_account_pubkey, false),
            AccountMeta::new_readonly(evm_loader_id, false),
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
