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
use crate::rpc::db::PostgresClient;
use std::any::Any;


pub trait RpcToAny: 'static {
    fn as_any(&self) -> &dyn Any;
}

impl<T: 'static> RpcToAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
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

impl Rpc for RpcClient{
    fn commitment(&self) -> CommitmentConfig {
        self.commitment()
    }

    fn confirm_transaction_with_spinner(&self, signature: &Signature, recent_blockhash: &Hash, commitment_config: CommitmentConfig) -> ClientResult<()>{
        self.confirm_transaction_with_spinner(signature, recent_blockhash, commitment_config)
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        self.get_account(key)
    }

    fn get_account_with_commitment(&self, key: &Pubkey, commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        self.get_account_with_commitment(key, commitment)
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        self.get_block(slot)
    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>{
        self.get_block_time(slot)
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        self.get_latest_blockhash()
    }

    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64>{
        self.get_minimum_balance_for_rent_exemption(data_len)
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        self.get_slot()
    }

    fn get_signature_statuses(&self, signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        self.get_signature_statuses(signatures)
    }

    fn get_transaction_with_config(&self, signature: &Signature, config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        self.get_transaction_with_config(signature, config)
    }

    fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature>{
        self.send_transaction(transaction)
    }

    fn send_and_confirm_transaction_with_spinner(&self, transaction: &Transaction) -> ClientResult<Signature>{
        self.send_and_confirm_transaction_with_spinner(transaction)
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, transaction: &Transaction, commitment: CommitmentConfig) -> ClientResult<Signature>{
        self.send_and_confirm_transaction_with_spinner_and_commitment(transaction, commitment)
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        self.send_and_confirm_transaction_with_spinner_and_config(transaction, commitment, config)
    }

    fn get_latest_blockhash_with_commitment(&self, commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        self.get_latest_blockhash_with_commitment(commitment)
    }
}


impl Rpc for PostgresClient {
    fn commitment(&self) -> CommitmentConfig {
        CommitmentConfig::default()
    }

    fn confirm_transaction_with_spinner(&self, _signature: &Signature, _recent_blockhash: &Hash, _commitment_config: CommitmentConfig) -> ClientResult<()>{
        Err(ClientErrorKind::Custom("confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        self.get_account_at_slot(key)
            .map_err(|_| ClientError::from(ClientErrorKind::Custom("load account error".to_string())) )?
            .ok_or_else(|| ClientError::from(ClientErrorKind::Custom(format!("account not found {}", key))))
    }

    fn get_account_with_commitment(&self, key: &Pubkey, _commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        let account= self.get_account_at_slot(key)
            .map_err(|_| ClientError::from( ClientErrorKind::Custom("load account error".to_string())))?;
        let context = RpcResponseContext{slot: self.slot, api_version: None};
        Ok(Response {context, value: account})
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        let hash = self.get_block_hash(slot)
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
    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp>{
        self.get_block_time(slot)
            .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_block_time error".to_string())))
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        let blockhash =  self.get_latest_blockhash()
            .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_latest_blockhash error".to_string())))?;
        blockhash.parse::<Hash>()
            .map_err(|_| ClientError::from( ClientErrorKind::Custom("get_latest_blockhash parse error".to_string())))
    }

    fn get_minimum_balance_for_rent_exemption(&self, _data_len: usize) -> ClientResult<u64>{
        Err(ClientErrorKind::Custom("get_minimum_balance_for_rent_exemption() not implemented for rpc_db client".to_string()).into())
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        Ok(self.slot)
    }

    fn get_signature_statuses(&self, _signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        Err(ClientErrorKind::Custom("get_signature_statuses() not implemented for rpc_db client".to_string()).into())
    }

    fn get_transaction_with_config(&self, _signature: &Signature, _config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        Err(ClientErrorKind::Custom("get_transaction_with_config() not implemented for rpc_db client".to_string()).into())
    }

    fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_transaction() not implemented for rpc_db client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner() not implemented for rpc_db client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, _transaction: &Transaction, _commitment: CommitmentConfig) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for rpc_db client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_config() not implemented for rpc_db client".to_string()).into())
    }

    fn get_latest_blockhash_with_commitment(&self, _commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        Err(ClientErrorKind::Custom("get_latest_blockhash_with_commitment() not implemented for rpc_db client".to_string()).into())
    }
}
