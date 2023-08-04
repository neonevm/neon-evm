use super::{e, Rpc};
use crate::types::{ChDbConfig, IndexerDb, TracerDb, TxParams};
use async_trait::async_trait;
use solana_client::{
    client_error::Result as ClientResult,
    client_error::{ClientError, ClientErrorKind},
    rpc_config::{RpcSendTransactionConfig, RpcTransactionConfig},
    rpc_response::{Response, RpcResponseContext, RpcResult},
};
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

pub struct TrxDbClient {
    pub hash: [u8; 32],
    sol_sig: [u8; 64],
    tracer_db: TracerDb,
    indexer_db: IndexerDb,
}

impl TrxDbClient {
    pub async fn new(config: &ChDbConfig, hash: [u8; 32]) -> Self {
        let tracer_db = TracerDb::new(config);
        let indexer_db = IndexerDb::new(config).await;
        let sol_sig = indexer_db
            .get_sol_sig(&hash)
            .await
            .unwrap_or_else(|_| panic!("get_sol_sig error, hash: 0x{}", hex::encode(hash)));

        Self {
            hash,
            sol_sig,
            tracer_db,
            indexer_db,
        }
    }
}

#[async_trait]
impl Rpc for TrxDbClient {
    fn commitment(&self) -> CommitmentConfig {
        CommitmentConfig::default()
    }

    async fn confirm_transaction_with_spinner(
        &self,
        _signature: &Signature,
        _recent_blockhash: &Hash,
        _commitment_config: CommitmentConfig,
    ) -> ClientResult<()> {
        Err(e!(
            "confirm_transaction_with_spinner() not implemented for trx rpc client"
        ))
    }

    async fn get_account(&self, key: &Pubkey) -> ClientResult<Account> {
        self.tracer_db
            .get_account_by_sol_sig(key, &self.sol_sig)
            .await
            .map_err(|e| e!("load account error", key, e))?
            .ok_or_else(|| e!("account not found", key))
    }

    async fn get_account_with_commitment(
        &self,
        key: &Pubkey,
        _commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        let account = self
            .tracer_db
            .get_account_by_sol_sig(key, &self.sol_sig)
            .await
            .map_err(|e| e!("load account error", key, e))?;

        let slot = self
            .indexer_db
            .get_slot(&self.hash)
            .await
            .map_err(|e| e!("get_slot error", e))?;

        let context = RpcResponseContext {
            slot,
            api_version: None,
        };
        Ok(Response {
            context,
            value: account,
        })
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        let mut result = Vec::new();
        for key in pubkeys {
            let account = self
                .tracer_db
                .get_account_by_sol_sig(key, &self.sol_sig)
                .await
                .map_err(|e| e!("load account error", key, e))?;
            result.push(account);
        }
        Ok(result)
    }

    async fn get_account_data(&self, key: &Pubkey) -> ClientResult<Vec<u8>> {
        Ok(self.get_account(key).await?.data)
    }

    async fn get_block(&self, _slot: Slot) -> ClientResult<EncodedConfirmedBlock> {
        Err(e!("get_block() not implemented for db_trx_client"))
    }

    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.tracer_db
            .get_block_time(slot)
            .await
            .map_err(|e| e!("get_block_time error", slot, e))
    }

    async fn get_latest_blockhash(&self) -> ClientResult<Hash> {
        Err(e!(
            "get_latest_blockhash() not implemented for db_trx_client"
        ))
    }

    async fn get_minimum_balance_for_rent_exemption(&self, _data_len: usize) -> ClientResult<u64> {
        Err(e!(
            "get_minimum_balance_for_rent_exemption() not implemented for db_trx_client"
        ))
    }

    async fn get_slot(&self) -> ClientResult<Slot> {
        self.indexer_db
            .get_slot(&self.hash)
            .await
            .map_err(|e| e!("get_slot error", e))
    }

    async fn get_signature_statuses(
        &self,
        _signatures: &[Signature],
    ) -> RpcResult<Vec<Option<TransactionStatus>>> {
        Err(e!(
            "get_signature_statuses() not implemented for db_trx_client"
        ))
    }

    async fn get_transaction_with_config(
        &self,
        _signature: &Signature,
        _config: RpcTransactionConfig,
    ) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta> {
        Err(e!(
            "get_transaction_with_config() not implemented for db_trx_client"
        ))
    }

    async fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature> {
        Err(e!("send_transaction() not implemented for db_trx_client"))
    }

    async fn send_and_confirm_transaction_with_spinner(
        &self,
        _transaction: &Transaction,
    ) -> ClientResult<Signature> {
        Err(e!(
            "send_and_confirm_transaction_with_spinner() not implemented for db_trx_client"
        ))
    }

    async fn send_and_confirm_transaction_with_spinner_and_commitment(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
    ) -> ClientResult<Signature> {
        Err(e!("send_and_confirm_transaction_with_spinner_and_commitment() not implemented for db_trx_client"))
    }

    async fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        Err(e!("send_and_confirm_transaction_with_spinner_and_config() not implemented for db_trx_client"))
    }

    async fn get_latest_blockhash_with_commitment(
        &self,
        _commitment: CommitmentConfig,
    ) -> ClientResult<(Hash, u64)> {
        Err(e!(
            "get_latest_blockhash_with_commitment() not implemented for db_trx_client"
        ))
    }

    async fn get_transaction_data(&self) -> ClientResult<TxParams> {
        self.indexer_db
            .get_transaction_data(&self.hash)
            .await
            .map_err(|e| e!("load transaction error", self.hash, e))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
