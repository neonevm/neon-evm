use super::Rpc;
use async_trait::async_trait;
use solana_client::{
    client_error::Result as ClientResult,
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcLargestAccountsConfig, RpcSimulateTransactionConfig},
    rpc_response::{RpcResult, RpcSimulateTransactionResult},
};
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    transaction::Transaction,
};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub struct CloneRpcClient(Arc<RpcClient>);

impl CloneRpcClient {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self(Arc::new(rpc_client))
    }
}

impl Deref for CloneRpcClient {
    type Target = RpcClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait(?Send)]
impl Rpc for CloneRpcClient {
    async fn get_account(&self, key: &Pubkey) -> RpcResult<Option<Account>> {
        self.0
            .get_account_with_commitment(key, self.commitment())
            .await
    }

    async fn get_account_with_commitment(
        &self,
        key: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        self.0.get_account_with_commitment(key, commitment).await
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        let mut result: Vec<Option<Account>> = Vec::new();
        for chunk in pubkeys.chunks(100) {
            let mut accounts = self.0.get_multiple_accounts(chunk).await?;
            result.append(&mut accounts);
        }

        Ok(result)
    }

    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.0.get_block_time(slot).await
    }

    async fn get_slot(&self) -> ClientResult<Slot> {
        self.0.get_slot().await
    }
}

#[async_trait(?Send)]
pub trait SolanaRpc {
    async fn simulate_transaction_with_instructions(
        &self,
        signer: Option<Pubkey>,
        instructions: &[Instruction],
    ) -> RpcResult<RpcSimulateTransactionResult>;

    async fn get_account_with_sol(&self) -> ClientResult<Pubkey>;
}

#[async_trait(?Send)]
impl SolanaRpc for RpcClient {
    async fn simulate_transaction_with_instructions(
        &self,
        signer: Option<Pubkey>,
        instructions: &[Instruction],
    ) -> RpcResult<RpcSimulateTransactionResult> {
        let payer_pubkey = if let Some(signer) = signer {
            signer
        } else {
            self.get_account_with_sol().await?
        };

        let tx = Transaction::new_with_payer(instructions, Some(&payer_pubkey));

        self.simulate_transaction_with_config(
            &tx,
            RpcSimulateTransactionConfig {
                sig_verify: false,
                replace_recent_blockhash: true,
                ..RpcSimulateTransactionConfig::default()
            },
        )
        .await
    }

    async fn get_account_with_sol(&self) -> ClientResult<Pubkey> {
        let r = self
            .get_largest_accounts_with_config(RpcLargestAccountsConfig {
                commitment: Some(self.commitment()),
                filter: None,
            })
            .await?;

        let pubkey = Pubkey::from_str(&r.value[0].address).unwrap();
        Ok(pubkey)
    }
}
