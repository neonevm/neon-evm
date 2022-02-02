use log::{debug, info};

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
    system_program,
    sysvar,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use evm::{H160};

use evm_loader::{
    instruction::EvmInstruction,
};

use crate::{
    Config,
    NeonCliResult,
};


pub fn execute (
    config: &Config,
    ether_address: &H160,
    lamports: u64,
    space: u64,
    token_mint: &Pubkey
) -> NeonCliResult {

    let (solana_address, nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);
    let token_address = spl_associated_token_account::get_associated_token_address(&solana_address, token_mint);
    debug!("Create ethereum account {} <- {} {}", solana_address, hex::encode(ether_address), nonce);

    let instruction = Instruction::new_with_bincode(
            config.evm_loader,
            &EvmInstruction::CreateAccount {lamports, space, ether: *ether_address, nonce},
            vec![
                AccountMeta::new(config.signer.pubkey(), true),
                AccountMeta::new(solana_address, false),
                AccountMeta::new(token_address, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(*token_mint, false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(spl_associated_token_account::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ]);

    let finalize_message = Message::new(&[instruction], Some(&config.signer.pubkey()));
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
        "solana": solana_address.to_string(),
        "token": token_address.to_string(),
        "ether": hex::encode(ether_address),
        "nonce": nonce,
    }));

    Ok(())
}

