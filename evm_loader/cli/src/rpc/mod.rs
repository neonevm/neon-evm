pub mod db_call_client;
pub mod db_trx_client;
mod validator_client;

use solana_client::{
    client_error::{Result as ClientResult,},
    rpc_config::{RpcTransactionConfig, RpcSendTransactionConfig},
    rpc_response::RpcResult,
};
use solana_sdk::{
    account::Account, pubkey::Pubkey, commitment_config::CommitmentConfig, clock::{UnixTimestamp, Slot},
    hash::Hash, signature::Signature, transaction::Transaction,
};
use solana_transaction_status::{EncodedConfirmedBlock, EncodedConfirmedTransactionWithStatusMeta, TransactionStatus};
use crate::types::TxParams;
use std::any::Any;

pub trait Rpc{
    fn commitment(&self) -> CommitmentConfig;
    fn confirm_transaction_with_spinner(&self, signature: &Signature, recent_blockhash: &Hash, commitment_config: CommitmentConfig) -> ClientResult<()>;
    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>;
    fn get_account_with_commitment(&self, key: &Pubkey, commitment: CommitmentConfig) -> RpcResult<Option<Account>>;
    fn get_account_data(&self, key: &Pubkey) -> ClientResult<Vec<u8>>;
    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>;
    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>;
    fn get_latest_blockhash(&self) -> ClientResult<Hash>;
    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64>;
    fn get_slot(&self) -> ClientResult<Slot>;
    fn get_signature_statuses(&self, signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>>;
    fn get_transaction_with_config(&self, signature: &Signature, config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>;
    fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature>;
    fn send_and_confirm_transaction_with_spinner(&self, transaction: &Transaction) -> ClientResult<Signature>;
    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, transaction: &Transaction, commitment: CommitmentConfig) -> ClientResult<Signature>;
    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>;
    fn get_latest_blockhash_with_commitment(&self, commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>;
    fn get_transaction_data(&self) -> ClientResult<TxParams>;
    fn as_any(&self) -> &dyn Any;
}

macro_rules! e {
    ($mes:expr) => {
        ClientError::from(
            ClientErrorKind::Custom(format!("{}", $mes))
        )
    };
    ($mes:expr, $error:expr) => {
        ClientError::from(
            ClientErrorKind::Custom(format!("{}: {:?}", $mes, $error))
        )
    };
    ($mes:expr, $error:expr, $arg:expr) => {
        ClientError::from(
            ClientErrorKind::Custom(format!("{}, {:?}: {:?}", $mes, $error, $arg))
        )
    };
}
pub(crate) use e;


