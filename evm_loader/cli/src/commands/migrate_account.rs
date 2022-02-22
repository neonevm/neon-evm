use log::{error, info, debug};

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use spl_associated_token_account::get_associated_token_address;

use evm::{H160};

use evm_loader::config::token_mint;

use crate::{
    Config,
    NeonCliError,
    NeonCliResult,
    make_solana_program_address,
};

/// Executes subcommand `migrate-account`.
#[allow(clippy::unnecessary_wraps)]
pub fn execute(
    config: &Config,
    ether_address: &H160,
) -> NeonCliResult {
    let (ether_pubkey, nonce) = make_solana_program_address(ether_address, &config.evm_loader);

    // Check existence of ether account
    config.rpc_client.get_account(&ether_pubkey)
        .map_err(|e| {
            error!("{}", e);
            NeonCliError::AccountNotFoundAtAddress(*ether_address)
        })?;

    let instructions = vec![
        migrate_account_instruction(
            config,
            ether_pubkey,
    )];

    let finalize_message = Message::new(&instructions, Some(&config.signer.pubkey()));
    let (blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &fee_calculator,
        &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    info!("{}", serde_json::json!({
        "ether address": hex::encode(ether_address),
        "nonce": nonce,
    }));

    Ok(())
}

/// Returns instruction to migrate Ethereum account.
fn migrate_account_instruction(
    config: &Config,
    ether_pubkey: Pubkey,
) -> Instruction {
    let token_authority = Pubkey::find_program_address(&[b"Deposit"], &config.evm_loader).0;
    let token_pool_pubkey = get_associated_token_address(&token_authority, &token_mint::id());
    let ether_token_pubkey = get_associated_token_address(&ether_pubkey, &token_mint::id());

    dbg!(token_authority);
    dbg!(token_pool_pubkey);
    dbg!(ether_token_pubkey);

    Instruction::new_with_bincode(
        config.evm_loader,
        &(26_u8),
        vec![
            AccountMeta::new(config.signer.pubkey(), false),
            AccountMeta::new(ether_pubkey, false),
            AccountMeta::new(ether_token_pubkey, false),
            AccountMeta::new(token_pool_pubkey, false),
            AccountMeta::new_readonly(token_authority, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
    )
}
