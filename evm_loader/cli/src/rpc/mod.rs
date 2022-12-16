mod call_db;
mod trx_db;

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
use crate::{rpc::db::PostgresClient, types::TxMeta};
use std::any::Any;
use evm_loader::H256;
use tokio::task::block_in_place;


#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DbConfig{
    pub tracer_host: String,
    pub tracer_port: String,
    pub tracer_database: String,
    pub tracer_user: String,
    pub tracer_password: String,
    pub indexer_host: String,
    pub indexer_port: String,
    pub indexer_database: String,
    pub indexer_user: String,
    pub indexer_password: String,
}

pub trait ToAny: 'static {
    fn as_any(&self) -> &dyn Any;
}

impl<T: 'static> ToAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}


pub trait DbClient {
    fn get_account_at_slot(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error>;
    fn get_block_hash(&self, slot: u64) -> Result<String, Error>;
    fn get_block_time(&self, slot: u64) -> Result<i64, Error>;
    fn get_latest_blockhash(&self) -> Result<String, Error>;
    // fn get_transaction_data(&self, hash: &String) -> Result<>;
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
    fn get_transaction_data(&self, tx: H256) -> ClientResult<Vec<TxMeta<SanitizedMessage>>>;
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

    fn get_transaction_data(&self, tx: H256) -> ClientResult<Vec<TxMeta<SanitizedMessage>>> {
        Err(ClientErrorKind::Custom("get_transaction_data() not implemented for rpc_node client".to_string()).into())
    }

}



pub fn do_connect(host: &String, port: &String, db: &String, user: &String, pass: &String) -> Client {
    let authority= format!(
        "host={} port={} dbname={} user={} password={}", host, port, db, user, pass
    );

    let mut attempt = 0;
    let mut result = None;

    while attempt < 3 {
        result = block(|| async {
            connect(&authority, NoTls).await
        }).ok();
        if result.is_some() {
            break;
        }
        attempt += 1;
    }

    let (client, connection) = result.expect("error to set DB connection");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    client
}



pub fn block<F, Fu, R>(f: F) -> R
    where
        F: FnOnce() -> Fu,
        Fu: std::future::Future<Output = R>,
{
    block_in1_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(f())
    })
}
