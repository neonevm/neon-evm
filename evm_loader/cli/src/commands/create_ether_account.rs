use log::{debug, info};

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    transaction::Transaction,
    system_program,
};

use solana_cli::{
    checks::{check_account_for_fee},
};

use evm::{H160};

use crate::{
    Config,
    NeonCliResult,
};


pub fn execute (
    config: &Config,
    ether_address: &H160,
) -> NeonCliResult {

    let (solana_address, nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);
    debug!("Create ethereum account {} <- {} {}", solana_address, hex::encode(ether_address), nonce);

    let create_account_v03_instruction = Instruction::new_with_bincode(
        config.evm_loader,
        &(0x28_u8, ether_address.as_fixed_bytes()),
        vec![
            AccountMeta::new(config.signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(solana_address, false),
        ]
    );

    let instructions = vec![create_account_v03_instruction];

    let mut finalize_message = Message::new(&instructions, Some(&config.signer.pubkey()));
    let blockhash = config.rpc_client.get_latest_blockhash()?;
    finalize_message.recent_blockhash = blockhash;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &finalize_message
    )?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    info!("{}", serde_json::json!({
        "solana": solana_address.to_string(),
        "ether": hex::encode(ether_address),
        "nonce": nonce,
    }));

    Ok(())
}

