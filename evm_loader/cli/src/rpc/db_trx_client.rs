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
use std::{convert::TryFrom, str::FromStr};
use evm_loader::{H160, H256, U256};
use super::{DbConfig, TrxDbClient, Rpc, block, do_connect, db_call_client::db_client_impl, e,};
use crate::commands::TxParams;
use std::{convert::TryInto, any::Any};


impl TrxDbClient {
    pub fn new(config: &DbConfig, hash: H256) -> Self {
        let tracer = do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        let indexer = do_connect(
            &config.indexer_host, &config.indexer_port, &config.indexer_database, &config.indexer_user, &config.indexer_password
        );
        Self {hash, tracer_db: tracer, indexer_db: indexer}
    }

    fn get_account_at_(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {

        let hex = format!("0x{}", hex::encode(self.hash.as_bytes()));
        let row = block(|| async {
            self.indexer_db.query_one(
                "SELECT S.sol_sig from solana_neon_transactions S, solana_blocks B \
                where S.block_slot = B.block_slot \
                and B.is_active =  true \
                and S.neon_sig = $1",
                &[&hex]
            ).await
        })?;
        let sol_sig_b58: &str = row.try_get(0)?;
        let sol_sig_b58 = sol_sig_b58.to_string();
        let sol_sig = bs58::decode(sol_sig_b58).into_vec().expect("sol_sig base58 decode error");
        let sol_sig: [u8; 64] = sol_sig.as_slice().try_into().unwrap();

        let pubkey_bytes = pubkey.to_bytes();
        let row = block(|| async {
            self.tracer_db.query_one(
                "SELECT * FROM get_pre_accounts($1, $2)",
                &[&sol_sig.as_slice(), &[&pubkey_bytes.as_slice()]]
            ).await
        })?;
        let lamports: i64 = row.try_get(0)?;
        let rent_epoch: i64 = row.try_get(4)?;

        let account = Account {
            lamports: u64::try_from(lamports).expect("lamports cast error"),
            data: row.try_get(1)?,
            owner: Pubkey::new(row.try_get(2)?),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).expect("rent_epoch cast error"),
        };

        Ok(Some(account))
    }

    pub fn get_slot_(&self) -> Result<Slot, Error>{
        let hex = format!("0x{}", hex::encode(self.hash.as_bytes()));
        let row = block(|| async {
            self.indexer_db.query_one(
                "SELECT min(S.block_slot) from solana_neon_transactions S, solana_blocks B \
                where S.block_slot = B.block_slot \
                and B.is_active =  true \
                and S.neon_sig = $1",
                &[&hex]
            ).await
        })?;
        let slot: i64 = row.try_get(0)?;
        Ok(u64::try_from(slot).expect("slot cast error"))
    }

    pub fn get_transaction_data_(&self) -> Result<TxParams, Error> {
        let hex = format!("0x{}", hex::encode(self.hash.as_bytes()));
        let row = block(|| async {
            self.indexer_db.query_one(
                "select distinct t.from_addr, \
                COALESCE(t.to_addr, t.contract), t.calldata, t.value, t.gas_limit \
                 from neon_transactions as t, solana_blocks as b \
                    where t.block_slot = b.block_slot \
                    and b.is_active =  true \
                    and t.neon_sig = $1",
                &[&hex]
            ).await
        })?;

        let from: String = row.try_get(0)?;
        let to: String = row.try_get(1)?;
        let data: String = row.try_get(2)?;
        let value: String = row.try_get(3)?;
        let gas_limit: String = row.try_get(4)?;

        let from = H160::from_str(&from.as_str()[2..]).expect("parse error from");
        let to = H160::from_str(&to.as_str()[2..]).expect("parse error to");
        let data =  hex::decode(&data.as_str()[2..]).expect("data hex::decore error");
        let value: U256 = value.as_str()[2..].parse().expect("value parse error");
        let gas_limit: U256 = gas_limit.as_str()[2..].parse().expect("gas_limit parse error");

        Ok(TxParams {from, to: Some(to), data: Some(data), value: Some(value), gas_limit: Some(gas_limit)})
    }

    db_client_impl!();
}


impl Rpc for TrxDbClient {
    fn commitment(&self) -> CommitmentConfig {
        CommitmentConfig::default()
    }

    fn confirm_transaction_with_spinner(&self, _signature: &Signature, _recent_blockhash: &Hash, _commitment_config: CommitmentConfig) -> ClientResult<()>{
        Err(e!("confirm_transaction_with_spinner() not implemented for trx rpc client"))
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account>  {
        self.get_account_at_(key)
            .map_err(|e| e!("load account error", key, e) )?
            .ok_or_else(|| e!("account not found", key))
    }

    fn get_account_with_commitment(&self, key: &Pubkey, _commitment: CommitmentConfig) -> RpcResult<Option<Account>> {
        let account= self.get_account_at_(key)
            .map_err(|e| e!("load account error", key, e))?;
        let slot = self.get_slot()?;
        let context = RpcResponseContext{slot, api_version: None};
        Ok(Response {context, value: account})
    }

    fn get_account_data(&self, key: &Pubkey)-> ClientResult<Vec<u8>>{
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock>{
        let hash = self.get_block_hash_(slot)
            .map_err(|e| e!("get_block error", slot, e))?;

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
        self.get_block_time_(slot)
            .map_err(|e| e!("get_block_time error", slot, e))
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash>{
        let hash =  self.get_latest_blockhash_().map_err(|e| e!("get_latest_blockhash error", e))?;
        hash.parse::<Hash>().map_err(|e| e!("get_latest_blockhash parse error", e))
    }

    fn get_minimum_balance_for_rent_exemption(&self, _data_len: usize) -> ClientResult<u64>{
        Err(e!("get_minimum_balance_for_rent_exemption() not implemented for db_trx_client"))
    }

    fn get_slot(&self) -> ClientResult<Slot>{
        self.get_slot_().map_err(|e| e!("get_slot error", e))
    }

    fn get_signature_statuses(&self, _signatures: &[Signature]) -> RpcResult<Vec<Option<TransactionStatus>>> {
        Err(e!("get_signature_statuses() not implemented for db_trx_client"))
    }

    fn get_transaction_with_config(&self, _signature: &Signature, _config: RpcTransactionConfig)-> ClientResult<EncodedConfirmedTransactionWithStatusMeta>{
        Err(e!("get_transaction_with_config() not implemented for db_trx_client"))
    }

    fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(e!("send_transaction() not implemented for db_trx_client"))
    }

    fn send_and_confirm_transaction_with_spinner(&self, _transaction: &Transaction) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner() not implemented for db_trx_client"))
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(&self, _transaction: &Transaction, _commitment: CommitmentConfig) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for db_trx_client"))
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig
    ) -> ClientResult<Signature>{
        Err(e!("send_and_confirm_transaction_with_spinner_and_config() not implemented for db_trx_client"))
    }

    fn get_latest_blockhash_with_commitment(&self, _commitment: CommitmentConfig) -> ClientResult<(Hash, u64)>{
        Err(e!("get_latest_blockhash_with_commitment() not implemented for db_trx_client"))
    }

    fn get_transaction_data(&self) -> ClientResult<TxParams> {
        self.get_transaction_data_().map_err(|e| e!("load transaction error", self.hash, e))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
