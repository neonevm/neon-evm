use super::Rpc;
use crate::rpc::block;
use crate::rpc::e;
use crate::rpc::Account;
use crate::rpc::ClientResult;
use crate::rpc::CommitmentConfig;
use crate::rpc::EncodedConfirmedBlock;
use crate::rpc::EncodedConfirmedTransactionWithStatusMeta;
use crate::rpc::Hash;
use crate::rpc::Pubkey;
use crate::rpc::RpcResult;
use crate::rpc::RpcSendTransactionConfig;
use crate::rpc::RpcTransactionConfig;
use crate::rpc::Signature;
use crate::rpc::Slot;
use crate::rpc::Transaction;
use crate::rpc::TransactionStatus;
use crate::rpc::TxParams;
use crate::rpc::UnixTimestamp;

use clickhouse::Client;
use solana_client::client_error::ClientError;
use solana_client::client_error::ClientErrorKind;
use std::any::Any;

#[allow(dead_code)]
struct ClickHouseClient {
    client: Client,
}

#[allow(dead_code)]
impl ClickHouseClient {
    pub fn _new(
        server_url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> ClickHouseClient {
        let client = match (username, password) {
            (None, None | Some(_)) => Client::default().with_url(server_url),
            (Some(user), None) => Client::default().with_url(server_url).with_user(user),
            (Some(user), Some(password)) => Client::default()
                .with_url(server_url)
                .with_user(user)
                .with_password(password),
        };

        ClickHouseClient { client }
    }

    pub fn get_block_time_(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        block(|| async {
            let query = "SELECT JSONExtractInt(notify_block_json, 'block_time') FROM events.notify_block_local WHERE (slot = toUInt64(?))";
            self.client
                .query(query)
                .bind(slot)
                .fetch_one::<UnixTimestamp>()
                .await
                .map_err(|e| e!("Failed to get block time, error: {}", e))
        })
    }

    pub fn get_block_(&self, _slot: Slot) -> ClientResult<EncodedConfirmedBlock> {
        let _ = self;
        todo!()
    }

    pub fn get_latest_blockhash_(&self) -> ClientResult<Hash> {
        block(|| async {
            let query =
                "SELECT hash FROM events.notify_block_local ORDER BY retrieved_time DESC LIMIT 1";
            let hash_string = self
                .client
                .query(query)
                .fetch_one::<String>()
                .await
                .map_err(|e| e!("Failed to get the latest blockhash, error: {}", e))?;

            let hash_vec = bs58::decode(hash_string)
                .into_vec()
                .map_err(|e| e!("Failed to decode the latest blockhash, error: {}", e))?;

            Ok(Hash::new(hash_vec.as_slice()))
        })
    }
}

impl Rpc for ClickHouseClient {
    fn commitment(&self) -> CommitmentConfig {
        todo!()
    }

    fn confirm_transaction_with_spinner(
        &self,
        _signature: &Signature,
        _recent_blockhash: &Hash,
        _commitment_config: CommitmentConfig,
    ) -> ClientResult<()> {
        todo!()
    }

    fn get_account(&self, _key: &Pubkey) -> ClientResult<Account> {
        todo!()
    }

    fn get_account_with_commitment(
        &self,
        _key: &Pubkey,
        _commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        todo!()
    }

    fn get_account_data(&self, _key: &Pubkey) -> ClientResult<Vec<u8>> {
        todo!()
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock> {
        self.get_block_(slot)
    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.get_block_time_(slot)
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash> {
        self.get_latest_blockhash_()
    }

    fn get_minimum_balance_for_rent_exemption(&self, _data_len: usize) -> ClientResult<u64> {
        todo!()
    }

    fn get_slot(&self) -> ClientResult<Slot> {
        todo!()
    }

    fn get_signature_statuses(
        &self,
        _signatures: &[Signature],
    ) -> RpcResult<Vec<Option<TransactionStatus>>> {
        todo!()
    }

    fn get_transaction_with_config(
        &self,
        _signature: &Signature,
        _config: RpcTransactionConfig,
    ) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta> {
        todo!()
    }

    fn send_transaction(&self, _transaction: &Transaction) -> ClientResult<Signature> {
        todo!()
    }

    fn send_and_confirm_transaction_with_spinner(
        &self,
        _transaction: &Transaction,
    ) -> ClientResult<Signature> {
        todo!()
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
    ) -> ClientResult<Signature> {
        todo!()
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        _transaction: &Transaction,
        _commitment: CommitmentConfig,
        _config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        todo!()
    }

    fn get_latest_blockhash_with_commitment(
        &self,
        _commitment: CommitmentConfig,
    ) -> ClientResult<(Hash, u64)> {
        todo!()
    }

    fn get_transaction_data(&self) -> ClientResult<TxParams> {
        todo!()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
