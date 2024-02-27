use crate::{config::APIOptions, Config};

use super::Rpc;
use async_trait::async_trait;
use solana_client::{
    client_error::Result as ClientResult, nonblocking::rpc_client::RpcClient,
    rpc_response::RpcResult,
};
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct CloneRpcClient {
    pub rpc: Arc<RpcClient>,
    pub key_for_config: Pubkey,
}

impl CloneRpcClient {
    pub fn new_from_config(config: &Config) -> Self {
        let url = config.json_rpc_url.clone();
        let commitment = config.commitment;

        let rpc_client = RpcClient::new_with_commitment(url, commitment);
        Self {
            rpc: Arc::new(rpc_client),
            key_for_config: config.key_for_config,
        }
    }

    pub fn new_from_api_config(config: &APIOptions) -> Self {
        let url = config.json_rpc_url.clone();
        let commitment = config.commitment;

        let rpc_client = RpcClient::new_with_commitment(url, commitment);
        Self {
            rpc: Arc::new(rpc_client),
            key_for_config: config.key_for_config,
        }
    }
}

impl Deref for CloneRpcClient {
    type Target = RpcClient;

    fn deref(&self) -> &Self::Target {
        &self.rpc
    }
}

#[async_trait(?Send)]
impl Rpc for CloneRpcClient {
    async fn get_account(&self, key: &Pubkey) -> RpcResult<Option<Account>> {
        self.rpc
            .get_account_with_commitment(key, self.commitment())
            .await
    }

    async fn get_account_with_commitment(
        &self,
        key: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        self.rpc.get_account_with_commitment(key, commitment).await
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        let mut result: Vec<Option<Account>> = Vec::new();
        for chunk in pubkeys.chunks(100) {
            let mut accounts = self.rpc.get_multiple_accounts(chunk).await?;
            result.append(&mut accounts);
        }

        Ok(result)
    }

    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.rpc.get_block_time(slot).await
    }

    async fn get_slot(&self) -> ClientResult<Slot> {
        self.rpc.get_slot().await
    }
}
