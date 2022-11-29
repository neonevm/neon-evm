pub mod db;

use solana_client::{
    rpc_client::RpcClient,
    client_error::{
        Result as ClientResult,
    },
    rpc_config::{RpcTransactionConfig, RpcSendTransactionConfig},
    rpc_response::{RpcResult, Response, RpcResponseContext},
    client_error::{ClientErrorKind, ClientError}
};

use solana_sdk::{
    account::Account, pubkey::Pubkey, commitment_config::CommitmentConfig, clock::{UnixTimestamp, Slot},
    hash::Hash, signature::Signature, transaction::Transaction,
};
use solana_transaction_status::{EncodedConfirmedBlock, EncodedConfirmedTransactionWithStatusMeta, TransactionStatus};
use std::sync::Arc;

use once_cell::sync::OnceCell;
use crate::rpc::db::PostgresClient;

pub static DB_INSTANCE: OnceCell<Arc<PostgresClient>> = OnceCell::new();
pub static NODE_INSTANCE: OnceCell<Arc<RpcClient>> = OnceCell::new();

pub struct DbClient;
pub struct NodeClient;

impl DbClient{
    pub fn global() -> &'static PostgresClient{
        DB_INSTANCE.get().expect("rpc_db client is not initialized")
    }
}

impl NodeClient{
    pub fn global() -> &'static RpcClient{
        NODE_INSTANCE.get().expect("rpc_node client is not initialized")
    }
}

pub enum Clients{
    Node,
    Postgress,
}

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
}

impl Rpc for Clients{
    fn commitment(&self) -> CommitmentConfig {
        match self{
            Clients::Node =>  NodeClient::global().commitment(),
            Clients::Postgress => CommitmentConfig::default(),
        }
    }

    fn confirm_transaction_with_spinner(&self, signature: &Signature, recent_blockhash: &Hash, commitment_config: CommitmentConfig) -> ClientResult<()>{
        match self{
            Clients::Node =>  {
                NodeClient::global().confirm_transaction_with_spinner(signature, recent_blockhash, commitment_config)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        match self{
            Clients::Node =>  {
                NodeClient::global().get_account(key)
            },
            Clients::Postgress => {
                DbClient::global().get_account_at_slot(key)
                    .map_err(|_| ClientError::from(ClientErrorKind::Custom("load account error".to_string())) )?
                    .ok_or_else(|| ClientError::from(ClientErrorKind::Custom(format!("account not found {}", key))))
            },
        }
    }

    fn get_account_with_commitment(&self, key: &Pubkey, commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        match self{
            Clients::Node =>  {
                NodeClient::global().get_account_with_commitment(key, commitment)
            },
            Clients::Postgress => {
                let account= DbClient::global().get_account_at_slot(key)
                    .map_err(|_| ClientError::from( ClientErrorKind::Custom("load account error".to_string())))?;
                let context = RpcResponseContext{slot: DbClient::global().slot, api_version: None};
                Ok(Response {context, value: account})
            },
        }
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        match self{
            Clients::Node => {
                NodeClient::global().get_block(slot)
            },
            Clients::Postgress =>  {
                let hash = DbClient::global().get_block_hash(slot)
                    .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_block_hash error".to_string())))?;

                Ok(EncodedConfirmedBlock{
                    previous_blockhash: String::default(),
                    blockhash: hash,
                    parent_slot: u64::default(),
                    transactions: vec![],
                    rewards: vec![],
                    block_time: None,
                    block_height: None,
                })
            },
        }

    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>{
        match self{
            Clients::Node => {
                NodeClient::global().get_block_time(slot)
            },
            Clients::Postgress =>  {
                DbClient::global().get_block_time(slot)
                    .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_block_time error".to_string())))
            },
        }
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        match self{
            Clients::Node => {
                NodeClient::global().get_latest_blockhash()
            },
            Clients::Postgress =>  {
                let blockhash =  DbClient::global().get_latest_blockhash()
                    .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_latest_blockhash error".to_string())))?;
                blockhash.parse::<Hash>()
                    .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_latest_blockhash parse error".to_string())))
            },
        }
    }

    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64>{
        match self{
            Clients::Node =>  {
                NodeClient::global().get_minimum_balance_for_rent_exemption(data_len)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("get_minimum_balance_for_rent_exemption() not implemented for rpc_db client".to_string()).into())
            },
        }

    }

    fn get_slot(&self) -> ClientResult<Slot>{
        match self{
            Clients::Node => {
                NodeClient::global().get_slot()
            },
            Clients::Postgress => {
                Ok(DbClient::global().slot)
            },
        }
    }

    fn get_signature_statuses(&self, signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        match self{
            Clients::Node => {
                NodeClient::global().get_signature_statuses(signatures)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("get_signature_statuses() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn get_transaction_with_config(&self, signature: &Signature, config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        match self{
            Clients::Node => {
                NodeClient::global().get_transaction_with_config(signature, config)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("get_transaction_with_config() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature>{
        match self{
            Clients::Node =>  {
                NodeClient::global().send_transaction(transaction)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("send_transaction() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn send_and_confirm_transaction_with_spinner(&self, transaction: &Transaction) -> ClientResult<Signature>{
        match self{
            Clients::Node => {
                NodeClient::global().send_and_confirm_transaction_with_spinner(transaction)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, transaction: &Transaction, commitment: CommitmentConfig) -> ClientResult<Signature>{
        match self{
            Clients::Node => {
                NodeClient::global().send_and_confirm_transaction_with_spinner_and_commitment(transaction, commitment)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        match self{
            Clients::Node => {
                NodeClient::global().send_and_confirm_transaction_with_spinner_and_config(transaction, commitment, config)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_config() not implemented for rpc_db client".to_string()).into())
            },
        }
    }

    fn get_latest_blockhash_with_commitment(&self, commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        match self{
            Clients::Node => {
                NodeClient::global().get_latest_blockhash_with_commitment(commitment)
            },
            Clients::Postgress => {
                Err(ClientErrorKind::Custom("get_latest_blockhash_with_commitment() not implemented for rpc_db client".to_string()).into())
            },
        }
    }
}
