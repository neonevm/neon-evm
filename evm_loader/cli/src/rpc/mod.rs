pub mod db;

use db::PostgresClient;
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

pub struct Clients{
    pub rpc_node: Arc<RpcClient>,
    pub rpc_db: Option<PostgresClient>,
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
        if self.rpc_db.is_some() {
            return CommitmentConfig::default()
        }
        self.rpc_node.commitment()
    }

    fn confirm_transaction_with_spinner(&self, signature: &Signature, recent_blockhash: &Hash, commitment_config: CommitmentConfig) -> ClientResult<()>{
        if self.rpc_db.is_some() {
            return Err(ClientErrorKind::Custom("confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.confirm_transaction_with_spinner(signature, recent_blockhash, commitment_config)
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        if self.rpc_db.is_some() {
            return self.rpc_db.as_ref().unwrap()
                .get_account_at_slot(key)
                .map_err(|_| ClientError::from(ClientErrorKind::Custom("load account error".to_string())) )?
                .ok_or_else(|| ClientError::from(ClientErrorKind::Custom(format!("account not found {}", key))))
        }
        self.rpc_node.get_account(key)
    }

    fn get_account_with_commitment(&self, key: &Pubkey, commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        if self.rpc_db.is_some(){
            let rpc_db = self.rpc_db.as_ref().unwrap();
            let account= rpc_db.get_account_at_slot(key)
                .map_err(|_| ClientError::from( ClientErrorKind::Custom("load account error".to_string())))?;
            let context = RpcResponseContext{slot: rpc_db.slot, api_version: None};
            return Ok(Response {context, value: account})
        }

        self.rpc_node.get_account_with_commitment(key, commitment)
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        if self.rpc_db.is_some(){
            return Ok(self.get_account(key)?.data)
        }
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        self.rpc_node.get_block(slot)
    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>{
        self.rpc_node.get_block_time(slot)
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        self.rpc_node.get_latest_blockhash()
    }

    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64>{
        self.rpc_node.get_minimum_balance_for_rent_exemption(data_len)
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        if self.rpc_db.is_some(){
            let client = self.rpc_db.as_ref().unwrap();
            return Ok(client.slot)
        }
        self.rpc_node.get_slot()
    }

    fn get_signature_statuses(&self, signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("get_signature_statuses() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.get_signature_statuses(signatures)
    }

    fn get_transaction_with_config(&self, signature: &Signature, config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("get_transaction_with_config() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.get_transaction_with_config(signature, config)
    }

    fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("send_transaction() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.send_transaction(transaction)
    }

    fn send_and_confirm_transaction_with_spinner(&self, transaction: &Transaction) -> ClientResult<Signature>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.send_and_confirm_transaction_with_spinner(transaction)
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, transaction: &Transaction, commitment: CommitmentConfig) -> ClientResult<Signature>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.send_and_confirm_transaction_with_spinner_and_commitment(transaction, commitment)
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_config() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.send_and_confirm_transaction_with_spinner_and_config(transaction, commitment, config)
    }

    fn get_latest_blockhash_with_commitment(&self, commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        if self.rpc_db.is_some(){
            return Err(ClientErrorKind::Custom("get_latest_blockhash_with_commitment() not implemented for rpc_db client".to_string()).into())
        }
        self.rpc_node.get_latest_blockhash_with_commitment(commitment)
    }

}

