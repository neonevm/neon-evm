use super::{e, Rpc};
use crate::types::TxParams;
use solana_client::{
    client_error::Result as ClientResult,
    client_error::{ClientError, ClientErrorKind},
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcTransactionConfig},
    rpc_response::RpcResult,
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

impl Rpc for RpcClient {
    fn commitment(&self) -> CommitmentConfig {
        self.commitment()
    }

    fn confirm_transaction_with_spinner(
        &self,
        signature: &Signature,
        recent_blockhash: &Hash,
        commitment_config: CommitmentConfig,
    ) -> ClientResult<()> {
        self.confirm_transaction_with_spinner(signature, recent_blockhash, commitment_config)
    }

    fn get_account(&self, key: &Pubkey) -> ClientResult<Account> {
        self.get_account(key)
    }

    fn get_account_with_commitment(
        &self,
        key: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        self.get_account_with_commitment(key, commitment)
    }

    fn get_account_data(&self, key: &Pubkey) -> ClientResult<Vec<u8>> {
        Ok(self.get_account(key)?.data)
    }

    fn get_block(&self, slot: Slot) -> ClientResult<EncodedConfirmedBlock> {
        self.get_block(slot)
    }

    fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.get_block_time(slot)
    }

    fn get_latest_blockhash(&self) -> ClientResult<Hash> {
        self.get_latest_blockhash()
    }

    fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> ClientResult<u64> {
        self.get_minimum_balance_for_rent_exemption(data_len)
    }

    fn get_slot(&self) -> ClientResult<Slot> {
        self.get_slot()
    }

    fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> RpcResult<Vec<Option<TransactionStatus>>> {
        self.get_signature_statuses(signatures)
    }

    fn get_transaction_with_config(
        &self,
        signature: &Signature,
        config: RpcTransactionConfig,
    ) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta> {
        self.get_transaction_with_config(signature, config)
    }

    fn send_transaction(&self, transaction: &Transaction) -> ClientResult<Signature> {
        self.send_transaction(transaction)
    }

    fn send_and_confirm_transaction_with_spinner(
        &self,
        transaction: &Transaction,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction_with_spinner(transaction)
    }

    fn send_and_confirm_transaction_with_spinner_and_commitment(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction_with_spinner_and_commitment(transaction, commitment)
    }

    fn send_and_confirm_transaction_with_spinner_and_config(
        &self,
        transaction: &Transaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction_with_spinner_and_config(transaction, commitment, config)
    }

    fn get_latest_blockhash_with_commitment(
        &self,
        commitment: CommitmentConfig,
    ) -> ClientResult<(Hash, u64)> {
        self.get_latest_blockhash_with_commitment(commitment)
    }

    fn get_transaction_data(&self) -> ClientResult<TxParams> {
        Err(e!(
            "get_transaction_data() not implemented for validator_client"
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
