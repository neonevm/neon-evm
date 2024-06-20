use crate::{config::APIOptions, Config};

use super::Rpc;
use async_trait::async_trait;
use solana_account_decoder::{UiAccount, UiAccountEncoding};
use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcAccountInfoConfig,
    rpc_request::RpcRequest,
    rpc_response::Response,
};
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    pubkey::Pubkey,
};
use std::{error::Error, ops::Deref, time::Duration};
use std::{future::Future, sync::Arc};
use tracing::debug;

fn should_retry(e: &ClientError) -> bool {
    let ClientErrorKind::Reqwest(reqwest_error) = e.kind() else {
        return false;
    };

    let Some(source) = reqwest_error.source() else {
        return false;
    };

    let Some(hyper_error) = source.downcast_ref::<hyper::Error>() else {
        return false;
    };

    if hyper_error.is_incomplete_message() {
        return true;
    }

    let Some(hyper_source) = hyper_error.source() else {
        return false;
    };

    let Some(io_error) = hyper_source.downcast_ref::<std::io::Error>() else {
        return false;
    };

    io_error.kind() == std::io::ErrorKind::ConnectionReset
}

async fn with_retries<F, Fut, R>(max_retries: usize, request: F) -> ClientResult<R>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ClientResult<R>>,
{
    for _ in 0..max_retries {
        match request().await {
            Ok(result) => return Ok(result),
            Err(error) if should_retry(&error) => {
                log::warn!("{}", error);
                log::warn!("Retrying...");
                continue;
            }
            Err(error) => return Err(error),
        }
    }

    Err(ClientErrorKind::Custom("Max number of retries exceeded".to_string()).into())
}

#[derive(Clone)]
pub struct CloneRpcClient {
    pub rpc: Arc<RpcClient>,
    pub key_for_config: Pubkey,
    pub max_retries: usize,
}

impl CloneRpcClient {
    #[must_use]
    pub fn new_from_config(config: &Config) -> Self {
        let url = config.json_rpc_url.clone();
        let commitment = config.commitment;

        let rpc_client = RpcClient::new_with_commitment(url, commitment);
        Self {
            rpc: Arc::new(rpc_client),
            key_for_config: config.key_for_config,
            max_retries: 10,
        }
    }

    #[must_use]
    pub fn new_from_api_config(config: &APIOptions) -> Self {
        let url = config.solana_url.clone();
        let commitment = config.commitment;
        let timeout = Duration::from_secs(config.solana_timeout);

        let rpc_client = RpcClient::new_with_timeout_and_commitment(url, timeout, commitment);
        Self {
            rpc: Arc::new(rpc_client),
            key_for_config: config.key_for_config,
            max_retries: config.solana_max_retries,
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
    async fn get_account(&self, key: &Pubkey) -> ClientResult<Option<Account>> {
        let request = || {
            let config = RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                commitment: Some(self.commitment()),
                data_slice: None,
                min_context_slot: None,
            };
            let params = serde_json::json!([key.to_string(), config]);

            self.send(RpcRequest::GetAccountInfo, params)
        };

        let response: serde_json::Value = with_retries(self.max_retries, request).await?;
        let response: Response<Option<UiAccount>> = serde_json::from_value(response)?;

        let account = response.value.and_then(|v| v.decode());
        Ok(account)
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        if pubkeys.is_empty() {
            return Ok(Vec::new());
        }

        if pubkeys.len() == 1 {
            let account = Rpc::get_account(self, &pubkeys[0]).await?;
            debug!(
                "get_multiple_accounts: single account pubkey={} account={:?}",
                pubkeys[0], account
            );
            return Ok(vec![account]);
        }

        let mut result: Vec<Option<Account>> = Vec::with_capacity(pubkeys.len());
        for chunk in pubkeys.chunks(100) {
            let request = || self.rpc.get_multiple_accounts(chunk);

            let mut accounts = with_retries(self.max_retries, request).await?;
            debug!(
                "get_multiple_accounts: chunk pubkey={:?} account={:?}",
                chunk, accounts
            );
            result.append(&mut accounts);
        }

        Ok(result)
    }

    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        with_retries(self.max_retries, || self.rpc.get_block_time(slot)).await
    }

    async fn get_slot(&self) -> ClientResult<Slot> {
        with_retries(self.max_retries, || self.rpc.get_slot()).await
    }

    async fn get_deactivated_solana_features(&self) -> ClientResult<Vec<Pubkey>> {
        use std::time::{Duration, Instant};
        use tokio::sync::Mutex;

        struct Cache {
            data: Vec<Pubkey>,
            timestamp: Instant,
        }

        static CACHE: Mutex<Option<Cache>> = Mutex::const_new(None);
        let mut cache = CACHE.lock().await;

        if let Some(cache) = cache.as_ref() {
            if cache.timestamp.elapsed() < Duration::from_secs(24 * 60 * 60) {
                return Ok(cache.data.clone());
            }
        }

        let feature_keys: Vec<Pubkey> = solana_sdk::feature_set::FEATURE_NAMES
            .keys()
            .copied()
            .collect();

        let features = Rpc::get_multiple_accounts(self, &feature_keys).await?;

        let mut result = Vec::with_capacity(feature_keys.len());
        for (pubkey, feature) in feature_keys.iter().zip(features) {
            let is_activated = feature
                .and_then(|a| solana_sdk::feature::from_account(&a))
                .and_then(|f| f.activated_at)
                .is_some();

            if !is_activated {
                result.push(*pubkey);
            }
        }

        cache.replace(Cache {
            data: result.clone(),
            timestamp: Instant::now(),
        });
        drop(cache);

        Ok(result)
    }
}
