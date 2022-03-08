//! Faucet Solana utilities module.

use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use eyre::{eyre, Result, WrapErr};
use tracing::info;

use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer as _;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;

use crate::{config, ethereum, id::ReqId};

lazy_static::lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::default());
}

/// Creates the signleton instance of RpcClient.
pub fn init_client() {
    tokio::task::spawn_blocking(|| {
        CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new_with_commitment(
            config::solana_url(),
            config::solana_commitment(),
        ))
    });
}

/// Checks connection with Solana.
pub async fn is_alive() -> bool {
    let ok =
        tokio::task::spawn_blocking(|| -> bool { get_client().get_block_height().is_ok() }).await;
    ok.unwrap_or(false)
}

/// Returns instance of RpcClient.
pub fn get_client() -> Arc<RpcClient> {
    CLIENT.lock().unwrap().0.clone()
}

/// Converts amount of tokens from whole value to fractions (usually 10E-9).
pub fn convert_whole_to_fractions(amount: u64) -> Result<u64> {
    let decimals = config::solana_token_mint_decimals();
    let factor = 10_u64
        .checked_pow(decimals as u32)
        .ok_or_else(|| eyre!("Overflow 10^{}", decimals))?;
    amount
        .checked_mul(factor)
        .ok_or_else(|| eyre!("Overflow {}*{}", amount, factor))
}

/// Deposits `amount` of tokens from main account to associated account.
/// When `in_fractions` == false, amount is treated as whole token amount.
/// When `in_fractions` == true, amount is treated as amount in galans (10E-9).
pub async fn deposit_token(
    id: &ReqId,
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

    let evm_token_authority = Pubkey::find_program_address(&[b"Deposit"], &evm_loader_id).0;
    let evm_pool_pubkey = spl_associated_token_account::get_associated_token_address(
        &evm_token_authority,
        &token_mint_id,
    );

    let ether_pubkey = ether_address_to_solana_pubkey(&ether_address, &evm_loader_id).0;

    let id = id.to_owned();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let client = get_client();
        let mut instructions = Vec::with_capacity(3);

        let ether_account = client.get_account(&ether_pubkey);
        if ether_account.is_err() {
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
        info!("{} evm_pool_pubkey = {}", id, evm_pool_pubkey);
        info!("{} evm_token_authority = {}", id, evm_token_authority);
        info!("{} signer_pubkey = {}", id, signer_pubkey);
        info!("{} amount = {}", id, amount);

        instructions.push(spl_approve_instruction(
            spl_token::id(),
            signer_token_pubkey,
            evm_token_authority,
            signer_pubkey,
            amount,
        ));

        instructions.push(deposit_instruction(
            signer_token_pubkey,
            evm_pool_pubkey,
            ether_pubkey,
            evm_token_authority,
            evm_loader_id,
            spl_token::id(),
        ));

        info!(
            "{} Creating message with {} instructions...",
            id,
            instructions.len()
        );
        let message = Message::new(&instructions, Some(&signer_pubkey));
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
fn ether_address_to_solana_pubkey(
    ether_address: &ethereum::Address,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &[config::solana_account_seed_version()],
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
    let (solana_address, nonce) = ether_address_to_solana_pubkey(&ether_address, &evm_loader_id);

    Instruction::new_with_bincode(
        evm_loader_id,
        &(24_u8, ether_address.as_fixed_bytes(), nonce),
        vec![
            AccountMeta::new(signer_pubkey, true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(solana_address, false),
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
    evm_token_authority: Pubkey,
    evm_loader_id: Pubkey,
    spl_token_id: Pubkey,
) -> Instruction {
    Instruction::new_with_bincode(
        evm_loader_id,
        &(25_u8), // Index of the Deposit instruction in EVM Loader
        vec![
            AccountMeta::new(source_pubkey, false),
            AccountMeta::new(destination_pubkey, false),
            AccountMeta::new(ether_account_pubkey, false),
            AccountMeta::new_readonly(evm_token_authority, false),
            AccountMeta::new_readonly(spl_token_id, false),
        ],
    )
}

struct Client(Arc<RpcClient>);

impl Default for Client {
    fn default() -> Client {
        Client(Arc::new(RpcClient::new(String::default())))
    }
}
