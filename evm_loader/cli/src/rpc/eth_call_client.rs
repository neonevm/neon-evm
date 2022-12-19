use solana_client::{
    rpc_client::RpcClient,
    client_error::Result as ClientResult,
    rpc_config::{RpcTransactionConfig, RpcSendTransactionConfig},
    rpc_response::{RpcResult, Response, RpcResponseContext},
    client_error::{ClientErrorKind, ClientError}
};
use solana_sdk::{
    account::Account, pubkey::Pubkey, commitment_config::CommitmentConfig, clock::{UnixTimestamp, Slot},
    hash::Hash, signature::Signature, transaction::Transaction,
};
use solana_transaction_status::{EncodedConfirmedBlock, EncodedConfirmedTransactionWithStatusMeta, TransactionStatus};
use tokio_postgres::{ connect, Error, Client};
use postgres::{ NoTls};
use std::convert::TryFrom;
use evm_loader::H256;
use super::{DbConfig, DbClient, CallDbClient, Rpc, block, do_connect,} ;


impl CallDbClient {
    pub fn new(config: &DbConfig, slot: u64) -> Self {
        let client = do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        Self {slot, tracer_db: client}
    }
}

impl DbClient for CallDbClient {
    fn get_account_at(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let pubkey_bytes = pubkey.to_bytes();
        let row = block(|| async {
            self.tracer_db.query_one(
                "SELECT * FROM get_account_at_slot($1, $2)",
                &[&pubkey_bytes.as_slice(), &(self.slot as i64)]
            ).await
        })?;

        let lamports: i64 = row.try_get(2)?;
        let rent_epoch: i64 = row.try_get(4)?;
        Ok(Some(Account {
            lamports: u64::try_from(lamports).expect("lamports parse error"),
            data: row.try_get(5)?,
            owner: Pubkey::new(row.try_get(1)?),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).expect("rent_epoch parse error"),
        }))
    }
}


impl Rpc for CallDbClient {
    fn commitment(&self) -> CommitmentConfig {
        CommitmentConfig::default()
    }

    fn confirm_transaction_with_spinner(&self, _signature: &Signature, _recent_blockhash: &Hash, _commitment_config: CommitmentConfig) -> ClientResult<()>{
        Err(ClientErrorKind::Custom("confirm_transaction_with_spinner() not implemented for eth_call rpc client".to_string()).into())
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        self.get_account_at(key)
            .map_err(|_| ClientError::from(ClientErrorKind::Custom("load account error".to_string())) )?
            .ok_or_else(|| ClientError::from(ClientErrorKind::Custom(format!("account not found {}", key))))
    }

    fn get_account_with_commitment(&self, key: &Pubkey, _commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        let account= self.get_account_at(key)
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
        Err(ClientErrorKind::Custom("get_minimum_balance_for_rent_exemption() not implemented for eth_call rpc client".to_string()).into())
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        Ok(self.slot)
    }

    fn get_signature_statuses(&self, _signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        Err(ClientErrorKind::Custom("get_signature_statuses() not implemented for eth_call rpc client".to_string()).into())
    }

    fn get_transaction_with_config(&self, _signature: &Signature, _config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        Err(ClientErrorKind::Custom("get_transaction_with_config() not implemented for eth_call rpc client".to_string()).into())
    }

    fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_transaction() not implemented for eth_call rpc client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner() not implemented for eth_call rpc client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, _transaction: &Transaction, _commitment: CommitmentConfig) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for eth_call rpc client".to_string()).into())
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        Err(ClientErrorKind::Custom("send_and_confirm_transaction_with_spinner_and_config() not implemented for eth_call rpc client".to_string()).into())
    }

    fn get_latest_blockhash_with_commitment(&self, _commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        Err(ClientErrorKind::Custom("get_latest_blockhash_with_commitment() not implemented for eth_call rpc client".to_string()).into())
    }

    fn get_transaction_data(&self, tx: H256) -> ClientResult<TrxRow> {
        Err(ClientErrorKind::Custom("get_transaction_data() not implemented for eth_call rpc client".to_string()).into())
    }
}



