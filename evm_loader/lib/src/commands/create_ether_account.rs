use log::debug;
use serde::Serialize;
use solana_cli::checks::check_account_for_fee;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    system_program,
    transaction::Transaction,
};

use evm_loader::types::Address;

use crate::{Config, Context, NeonResult};

#[derive(Serialize)]
pub struct CreateEtherAccountReturn {
    pub solana_address: String,
}

pub async fn execute(
    config: &Config,
    context: &Context,
    ether_address: &Address,
) -> NeonResult<CreateEtherAccountReturn> {
    let (solana_address, nonce) = ether_address.find_solana_address(&config.evm_loader);
    let signer = context.signer()?;
    debug!("Create ethereum account {solana_address} <- {ether_address} {nonce}");

    let create_account_v03_instruction = Instruction::new_with_bincode(
        config.evm_loader,
        &(0x28_u8, ether_address.as_bytes()),
        vec![
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(solana_address, false),
        ],
    );

    let instructions = vec![create_account_v03_instruction];

    let mut finalize_message = Message::new(&instructions, Some(&signer.pubkey()));
    let blockhash = context.rpc_client.get_latest_blockhash().await?;
    finalize_message.recent_blockhash = blockhash;

    let client = context
        .blocking_rpc_client
        .as_ref()
        .expect("Blocking RPC client not initialized");

    check_account_for_fee(client, &signer.pubkey(), &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    context
        .rpc_client
        .send_and_confirm_transaction_with_spinner(&finalize_tx)
        .await?;

    Ok(CreateEtherAccountReturn {
        solana_address: solana_address.to_string(),
    })
}
