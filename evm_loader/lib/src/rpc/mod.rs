mod db_call_client;
mod validator_client;

pub use db_call_client::CallDbClient;

use crate::{NeonError, NeonResult};
use async_trait::async_trait;
use solana_cli::cli::CliError;
use solana_client::{
    client_error::Result as ClientResult,
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcTransactionConfig},
    rpc_response::RpcResult,
};
use solana_sdk::message::Message;
use solana_sdk::native_token::lamports_to_sol;
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    commitment_config::CommitmentConfig,
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use solana_transaction_status::{
    EncodedConfirmedBlock, EncodedConfirmedTransactionWithStatusMeta, TransactionStatus,
};
use std::any::Any;

#[async_trait(?Send)]
pub trait Rpc {
    fn commitment(&self) -> CommitmentConfig;
    async fn confirm_transaction_with_spinner(
        &self,
        signature: &Signature,
        recent_blockhash: &Hash,
        commitment_config: CommitmentConfig,
    ) -> ClientResult<()>;
    async fn get_account(&self, key: &Pubkey) -> ClientResult<Account>;
    async fn get_account_with_commitment(
        &self,
        key: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>>;
    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey])
        -> ClientResult<Vec<Option<Account>>>;
    async fn get_account_data(&self, key: &Pubkey) -> ClientResult<Vec<u8>>;
    async fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>;
    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>;
    async fn get_latest_blockhash(&self) -> ClientResult<Hash>;
    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64>;
    async fn get_slot(&self) -> ClientResult<Slot>;
    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> RpcResult<Vec<Option<TransactionStatus>>>;
    async fn get_transaction_with_config(
        &self,
        signature: &Signature,
        config: RpcTransactionConfig,
    ) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta>;
    async fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature>;
    async fn send_and_confirm_transaction_with_spinner(
        &self,
        transaction: &Transaction,
    ) -> ClientResult<Signature>;
    async fn send_and_confirm_transaction_with_spinner_and_commitment(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
    ) -> ClientResult<Signature>;
    async fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature>;
    async fn get_latest_blockhash_with_commitment(
        &self,
        commitment: CommitmentConfig,
    ) -> ClientResult<(Hash, u64)>;

    fn as_any(&self) -> &dyn Any;
}

macro_rules! e {
    ($mes:expr) => {
        ClientError::from(ClientErrorKind::Custom(format!("{}", $mes)))
    };
    ($mes:expr, $error:expr) => {
        ClientError::from(ClientErrorKind::Custom(format!("{}: {:?}", $mes, $error)))
    };
    ($mes:expr, $error:expr, $arg:expr) => {
        ClientError::from(ClientErrorKind::Custom(format!(
            "{}, {:?}: {:?}",
            $mes, $error, $arg
        )))
    };
}
pub(crate) use e;

pub(crate) async fn check_account_for_fee(
    rpc_client: &RpcClient,
    account_pubkey: &Pubkey,
    message: &Message,
) -> NeonResult<()> {
    let fee = rpc_client.get_fee_for_message(message).await?;
    let balance = rpc_client.get_balance(account_pubkey).await?;
    if balance != 0 && balance >= fee {
        return Ok(());
    }

    Err(NeonError::CliError(CliError::InsufficientFundsForFee(
        lamports_to_sol(fee),
        *account_pubkey,
    )))
}
