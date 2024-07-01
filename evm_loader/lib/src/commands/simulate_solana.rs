use std::collections::HashSet;

use bincode::Options;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use solana_program_runtime::compute_budget::ComputeBudget;
use solana_runtime::runtime_config::RuntimeConfig;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::{SanitizedTransaction, Transaction, VersionedTransaction},
};

use crate::{
    rpc::Rpc,
    solana_simulator::{SolanaSimulator, SyncState},
    types::SimulateSolanaRequest,
    NeonResult,
};

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SimulateSolanaTransactionResult {
    pub error: Option<solana_sdk::transaction::TransactionError>,
    pub logs: Vec<String>,
    pub executed_units: u64,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SimulateSolanaResponse {
    transactions: Vec<SimulateSolanaTransactionResult>,
}

fn decode_transaction(data: &[u8]) -> NeonResult<VersionedTransaction> {
    let tx_result = bincode::options()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize::<VersionedTransaction>(data);

    if let Ok(tx) = tx_result {
        return Ok(tx);
    }

    let tx = bincode::options()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize::<Transaction>(data)?;

    Ok(tx.into())
}

fn address_table_lookups(txs: &[VersionedTransaction]) -> Vec<Pubkey> {
    let mut accounts: HashSet<Pubkey> = HashSet::<Pubkey>::new();
    for tx in txs {
        let Some(address_table_lookups) = tx.message.address_table_lookups() else {
            continue;
        };

        for alt in address_table_lookups {
            accounts.insert(alt.account_key);
        }
    }

    accounts.into_iter().collect()
}

fn account_keys(txs: &[SanitizedTransaction]) -> Vec<Pubkey> {
    let mut accounts: HashSet<Pubkey> = HashSet::<Pubkey>::new();
    for tx in txs {
        let keys = tx.message().account_keys();
        accounts.extend(keys.iter());
    }

    accounts.into_iter().collect()
}

fn runtime_config(request: &SimulateSolanaRequest) -> RuntimeConfig {
    let compute_units = request.compute_units.unwrap_or(1_400_000);
    let heap_size = request.heap_size.unwrap_or(256 * 1024);

    let mut compute_budget = ComputeBudget::new(compute_units);
    compute_budget.heap_size = heap_size;

    RuntimeConfig {
        compute_budget: Some(compute_budget),
        log_messages_bytes_limit: Some(100 * 1024),
        transaction_account_lock_limit: request.account_limit,
    }
}

pub async fn execute(
    rpc: &impl Rpc,
    request: SimulateSolanaRequest,
) -> NeonResult<SimulateSolanaResponse> {
    let verify = request.verify.unwrap_or(true);
    let config = runtime_config(&request);

    let mut simulator = SolanaSimulator::new_with_config(rpc, config, SyncState::Yes).await?;

    // Decode transactions from bytes
    let mut transactions: Vec<VersionedTransaction> = vec![];
    for data in request.transactions {
        let tx = decode_transaction(&data)?;
        transactions.push(tx);
    }

    // Download ALT
    let alt = address_table_lookups(&transactions);
    simulator.sync_accounts(rpc, &alt).await?;

    // Sanitize transactions (verify tx and decode ALT)
    let mut sanitized_transactions: Vec<SanitizedTransaction> = vec![];
    for tx in transactions {
        let sanitized = simulator.sanitize_transaction(tx, verify)?;
        sanitized_transactions.push(sanitized);
    }

    // Download accounts
    let accounts = account_keys(&sanitized_transactions);
    simulator.sync_accounts(rpc, &accounts).await?;

    // simulator.replace_blockhash(&request.blockhash.into());

    // Process transactions
    let mut results = Vec::new();
    for tx in sanitized_transactions {
        let r = simulator.process_transaction(request.blockhash.into(), tx)?;
        results.push(SimulateSolanaTransactionResult {
            error: r.status.err(),
            logs: r.log_messages.unwrap_or_default(),
            executed_units: r.executed_units,
        });
    }

    Ok(SimulateSolanaResponse {
        transactions: results,
    })
}
