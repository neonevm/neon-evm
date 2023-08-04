use log::debug;
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    system_program,
    transaction::Transaction,
};

use evm_loader::types::Address;

use crate::rpc::check_account_for_fee;
use crate::NeonResult;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEtherAccountReturn {
    pub solana_address: String,
}

pub async fn execute(
    rpc_client: &RpcClient,
    evm_loader: Pubkey,
    signer: &dyn Signer,
    ether_address: &Address,
) -> NeonResult<CreateEtherAccountReturn> {
    let (solana_address, nonce) = ether_address.find_solana_address(&evm_loader);
    debug!("Create ethereum account {solana_address} <- {ether_address} {nonce}");

    let create_account_v03_instruction = Instruction::new_with_bincode(
        evm_loader,
        &(0x28_u8, ether_address.as_bytes()),
        vec![
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(solana_address, false),
        ],
    );

    let instructions = vec![create_account_v03_instruction];

    let mut finalize_message = Message::new(&instructions, Some(&signer.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().await?;
    finalize_message.recent_blockhash = blockhash;

    check_account_for_fee(rpc_client, &signer.pubkey(), &finalize_message).await?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    rpc_client
        .send_and_confirm_transaction_with_spinner(&finalize_tx)
        .await?;

    Ok(CreateEtherAccountReturn {
        solana_address: solana_address.to_string(),
    })
}
