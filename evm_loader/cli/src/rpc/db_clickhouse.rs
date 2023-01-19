use super::Rpc;
use clickhouse::Client;
use std::any::Any;
use crate::rpc::TxParams;
use crate::rpc::ClientResult;
use crate::rpc::Hash;
use crate::rpc::CommitmentConfig;
use crate::rpc::Signature;
use crate::rpc::RpcSendTransactionConfig;
use crate::rpc::Transaction;
use crate::rpc::EncodedConfirmedTransactionWithStatusMeta;
use crate::rpc::RpcTransactionConfig;
use crate::rpc::TransactionStatus;
use crate::rpc::RpcResult;
use crate::rpc::Slot;
use crate::rpc::UnixTimestamp;
use crate::rpc::EncodedConfirmedBlock;
use crate::rpc::Pubkey;
use crate::rpc::Account;

#[allow(dead_code)]
struct ClickHouseClient {
    client: Client,
}

impl ClickHouseClient {
    pub fn _new(
        server_url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> ClickHouseClient {
        let client = match (username, password) {
            (None, None) | (None, Some(_)) => Client::default().with_url(server_url),
            (Some(user), None) => Client::default().with_url(server_url).with_user(user),
            (Some(user), Some(password)) => Client::default()
                .with_url(server_url)
                .with_user(user)
                .with_password(password),
        };

        ClickHouseClient { client }
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

    fn get_block(&self, _slot: Slot) -> ClientResult<EncodedConfirmedBlock> {
        todo!()
    }

    fn get_block_time(&self, _slot: Slot) -> ClientResult<UnixTimestamp> {
        todo!()
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash> {
        todo!()
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
