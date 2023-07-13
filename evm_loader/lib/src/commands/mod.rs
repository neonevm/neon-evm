use crate::context::Context;
use solana_client::{
    client_error::{ClientError, Result as SolanaClientResult},
    rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::Instruction,
    message::Message,
    signature::Signature,
    transaction::Transaction,
};

pub mod cancel_trx;
pub mod collect_treasury;
pub mod create_ether_account;
pub mod deposit;
pub mod emulate;
pub mod get_ether_account_data;
pub mod get_neon_elf;
pub mod get_storage_at;
pub mod init_environment;
pub mod trace;
mod transaction_executor;

pub async fn send_transaction(
    context: &Context,
    instructions: &[Instruction],
) -> SolanaClientResult<Signature> {
    let signer = context.signer().map_err(|_| ClientError {
        kind: solana_client::client_error::ClientErrorKind::Custom("Signer creation error".into()),
        request: None,
    })?;
    let message = Message::new(instructions, Some(&signer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    let signers = [&*signer];
    let (blockhash, _last_valid_slot) = context
        .rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await?;
    transaction.try_sign(&signers, blockhash)?;

    context
        .rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &transaction,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                ..RpcSendTransactionConfig::default()
            },
        )
        .await
}
