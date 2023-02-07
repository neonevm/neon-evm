use solana_client::{
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
use tokio_postgres::Error;
use std::{convert::TryFrom, any::Any};
use super::{DbConfig, CallDbClient, Rpc, block, do_connect, e} ;
use crate::commands::TxParams;


macro_rules! db_client_impl {
    () => (
        fn get_block_hash_(&self, slot: u64) -> Result<String, Error>{
            let hash = block(|| async {
                self.tracer_db.query_one(
                    "SELECT blockhash FROM public.block WHERE slot = $1", &[&(slot as i64)],
                ).await
            })?.try_get(0)?;

            Ok(hash)
        }

        fn get_block_time_(&self, slot: u64) -> Result<i64, Error>{
            let time = block(|| async {
                self.tracer_db.query_one(
                    "SELECT block_time FROM public.block WHERE slot = $1", &[&(slot as i64)],
                ).await
            })?.try_get(0)?;

            Ok(time)
        }

        fn get_latest_blockhash_(&self) -> Result<String, Error>{
            let slot: i64 = block(|| async {
                self.tracer_db.query_one("SELECT MAX(slot) FROM public.slot", &[]).await
            })?.try_get(0)?;

            self.get_block_hash_(u64::try_from(slot).expect("slot parse error"))
        }
    )
}
pub(crate) use db_client_impl;


impl CallDbClient {
    pub fn new(config: &DbConfig, slot: u64) -> Self {
        let client = do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        Self {slot, tracer_db: client}
    }

    fn get_account_at_(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let pubkey_bytes = pubkey.to_bytes();
        let slot: i64 = self.slot.try_into().expect("slot < i64::MAX");
        let row = block(|| async {
            self.tracer_db.query_one(
                "SELECT * FROM get_account_at_slot($1, $2)",
                &[&pubkey_bytes.as_slice(), &slot]
            ).await
        })?;

        let owner: &[u8] = row.try_get(1)?;
        let lamports: i64 = row.try_get(2)?;
        let rent_epoch: i64 = row.try_get(4)?;
        Ok(Some(Account {
            lamports: u64::try_from(lamports).expect("lamports cast error"),
            data: row.try_get(5)?,
            owner: Pubkey::try_from(owner).expect("pubkey cast error"),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).expect("rent_epoch cast error"),
        }))
    }

    db_client_impl!();
}

impl Rpc for CallDbClient {
    fn commitment(&self) -> CommitmentConfig {
        CommitmentConfig::default()
    }

    fn confirm_transaction_with_spinner(&self, _signature: &Signature, _recent_blockhash: &Hash, _commitment_config: CommitmentConfig) -> ClientResult<()>{
        Err(e!("confirm_transaction_with_spinner() not implemented for db_call_client"))
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        self.get_account_at_(key)
            .map_err(|e| e!("load account error", key, e))?
            .ok_or_else(|| e!("account not found", key))
    }

    fn get_account_with_commitment(&self, key: &Pubkey, _commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        let account= self.get_account_at_(key)
            .map_err(|e| e!("load account error", key, e))?;
        let context = RpcResponseContext{slot: self.slot, api_version: None};
        Ok(Response {context, value: account})
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        let hash = self.get_block_hash_(slot).map_err(|e| e!("get_block error", slot, e))?;

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
        self.get_block_time_(slot).map_err(|e| e!("get_block_time error", slot, e))
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        let hash =  self.get_latest_blockhash_().map_err(|e| e!("get_latest_blockhash error", e))?;
        hash.parse::<Hash>().map_err(|e| e!("get_latest_blockhash parse error", e))
    }

    fn get_minimum_balance_for_rent_exemption(&self, _data_len: usize) -> ClientResult<u64>{
        Err(e!("get_minimum_balance_for_rent_exemption() not implemented for db_call_client"))
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        Ok(self.slot)
    }

    fn get_signature_statuses(&self, _signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        Err(e!("get_signature_statuses() not implemented for db_call_client"))
    }

    fn get_transaction_with_config(&self, _signature: &Signature, _config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        Err(e!("get_transaction_with_config() not implemented for db_call_client"))
    }

    fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(e!("send_transaction() not implemented for db_call_client"))
    }

    fn send_and_confirm_transaction_with_spinner(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner() not implemented for db_call_client"))
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, _transaction: &Transaction, _commitment: CommitmentConfig) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for db_call_client"))
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner_and_config() not implemented for db_call_client"))
    }

    fn get_latest_blockhash_with_commitment(&self, _commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        Err(e!("get_latest_blockhash_with_commitment() not implemented for db_call_client"))
    }

    fn get_transaction_data(&self) -> ClientResult<TxParams> {
        Err(e!("get_transaction_data() not implemented for db_call_client"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}



