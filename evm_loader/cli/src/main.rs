#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]

mod config;
mod logs;
mod program_options;

use neon_lib::{
    commands::{
        cancel_trx, collect_treasury, create_ether_account, deposit, emulate,
        get_ether_account_data, get_neon_elf, get_neon_elf::CachedElfParams, get_storage_at,
        init_environment, trace,
    },
    errors, rpc, types,
};

use clap::ArgMatches;
pub use config::Config;
use std::io::Read;

use ethnum::U256;
use evm_loader::evm::tracing::{TraceCallConfig, TraceConfig};
use serde_json::json;
use solana_clap_utils::input_parsers::{pubkey_of, value_of, values_of};
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::time::Instant;

use hex::FromHex;
use neon_lib::context::truncate_0x;
pub use neon_lib::context::*;
use neon_lib::rpc::CallDbClient;
use neon_lib::rpc::TrxDbClient;

use crate::{
    errors::NeonError,
    rpc::Rpc,
    types::{IndexerDb, TraceNextBlockParams, TransactionHashParams, TransactionParams, TxParams},
};
use evm_loader::types::Address;

type NeonCliResult = Result<serde_json::Value, NeonError>;

async fn run<'a>(options: &'a ArgMatches<'a>) -> NeonCliResult {
    let slot: Option<u64> = options
        .value_of("slot")
        .map(|slot_str| slot_str.parse().expect("slot parse error"));

    let config = config::create(options)?;

    let (cmd, params) = options.subcommand();

    let rpc_client: Box<dyn rpc::Rpc> = match (cmd, params) {
        ("emulate-hash" | "trace-hash" | "emulate_hash" | "trace_hash", Some(params)) => {
            let hash = params.value_of("hash").expect("hash not found");
            let hash = <[u8; 32]>::from_hex(truncate_0x(hash)).expect("hash cast error");

            Box::new(
                TrxDbClient::new(
                    config.db_config.as_ref().expect("db-config not found"),
                    hash,
                )
                .await,
            )
        }
        _ => {
            if let Some(slot) = slot {
                Box::new(CallDbClient::new(
                    config.db_config.as_ref().expect("db-config not found"),
                    slot,
                ))
            } else {
                Box::new(RpcClient::new_with_commitment(
                    config.json_rpc_url.clone(),
                    config.commitment,
                ))
            }
        }
    };

    let context: Context = create(&*rpc_client, &config);

    execute(cmd, params, &config, &context, slot).await
}

fn print_result(result: &NeonCliResult) {
    let logs = {
        let context = logs::CONTEXT.lock().unwrap();
        context.clone()
    };

    let result = match result {
        Ok(value) => serde_json::json!({
            "result": "success",
            "value": value,
            "logs": logs
        }),
        Err(e) => serde_json::json!({
            "result": "error",
            "error": e.to_string(),
            "logs": logs
        }),
    };

    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let time_start = Instant::now();

    let options = program_options::parse();

    logs::init(&options).expect("logs init error");
    std::panic::set_hook(Box::new(|info| {
        let message = std::format!("Panic: {info}");
        print_result(&Err(NeonError::Panic(message)));
    }));

    let result = run(&options).await;

    let execution_time = Instant::now().duration_since(time_start);
    log::info!("execution time: {} sec", execution_time.as_secs_f64());
    print_result(&result);
    if let Err(e) = result {
        std::process::exit(e.error_code());
    };
}

#[allow(clippy::too_many_lines)]
async fn execute<'a>(
    cmd: &str,
    params: Option<&'a ArgMatches<'a>>,
    config: &'a Config,
    context: &'a Context<'_>,
    slot: Option<u64>,
) -> NeonCliResult {
    match (cmd, params) {
        ("emulate", Some(params)) => {
            let (tx, trace_call_config) = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params).await;
            emulate::execute(
                context.rpc_client,
                config.evm_loader,
                tx,
                token,
                chain,
                steps,
                config.commitment,
                &accounts,
                &solana_accounts,
                &trace_call_config.block_overrides,
                trace_call_config.state_overrides,
            )
            .await
            .map(|result| json!(result))
        }
        ("emulate-hash", Some(params)) => {
            let tx = context.rpc_client.get_transaction_data().await?;
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params).await;
            emulate::execute(
                context.rpc_client,
                config.evm_loader,
                tx,
                token,
                chain,
                steps,
                config.commitment,
                &accounts,
                &solana_accounts,
                &None,
                None,
            )
            .await
            .map(|result| json!(result))
        }
        ("trace", Some(params)) => {
            let (tx, trace_call_config) = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params).await;
            trace::trace_transaction(
                context.rpc_client,
                config.evm_loader,
                tx,
                token,
                chain,
                steps,
                config.commitment,
                &accounts,
                &solana_accounts,
                trace_call_config,
            )
            .await
            .map(|trace| json!(trace))
        }
        ("trace-hash", Some(params)) => {
            let (tx, trace_config) = parse_tx_hash(context.rpc_client).await;
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params).await;
            trace::trace_transaction(
                context.rpc_client,
                config.evm_loader,
                tx,
                token,
                chain,
                steps,
                config.commitment,
                &accounts,
                &solana_accounts,
                trace_config.into(),
            )
            .await
            .map(|trace| json!(trace))
        }
        ("trace-next-block", Some(params)) => {
            let slot = slot.expect("SLOT argument is not provided");
            let trace_block_params: Option<TraceNextBlockParams> = read_from_stdin()
                .unwrap_or_else(|err| {
                    panic!("Unable to parse `TraceBlockBySlotParams` from STDIN, error: {err:?}")
                });
            let trace_config = trace_block_params
                .map(|params| params.trace_config.unwrap_or_default())
                .unwrap_or_default();
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params).await;
            let indexer_db =
                IndexerDb::new(config.db_config.as_ref().expect("db-config is required")).await;
            let transactions = indexer_db
                .get_block_transactions(slot + 1)
                .await
                .map_err(|e| {
                    ClientError::from(ClientErrorKind::Custom(format!(
                        "get_block_transactions error: {e}"
                    )))
                })?;
            trace::trace_block(
                context.rpc_client,
                config.evm_loader,
                transactions,
                token,
                chain,
                steps,
                config.commitment,
                &accounts,
                &solana_accounts,
                &trace_config,
            )
            .await
            .map(|traces| json!(traces))
        }
        ("create-ether-account", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            let rpc_client = context
                .rpc_client
                .as_any()
                .downcast_ref::<RpcClient>()
                .expect("cast to solana_client::nonblocking::rpc_client::RpcClient error");
            create_ether_account::execute(
                rpc_client,
                config.evm_loader,
                context.signer()?.as_ref(),
                &ether,
            )
            .await
            .map(|result| json!(result))
        }
        ("deposit", Some(params)) => {
            let rpc_client = context
                .rpc_client
                .as_any()
                .downcast_ref::<RpcClient>()
                .expect("cast to solana_client::nonblocking::rpc_client::RpcClient error");
            let amount = value_of(params, "amount").expect("amount parse error");
            let ether = address_of(params, "ether").expect("ether parse error");
            deposit::execute(
                rpc_client,
                config.evm_loader,
                context.signer()?.as_ref(),
                amount,
                &ether,
            )
            .await
            .map(|result| json!(result))
        }
        ("get-ether-account-data", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            get_ether_account_data::execute(context.rpc_client, &config.evm_loader, &ether)
                .await
                .map(|result| json!(result))
        }
        ("cancel-trx", Some(params)) => {
            let storage_account =
                pubkey_of(params, "storage_account").expect("storage_account parse error");
            cancel_trx::execute(
                context.rpc_client,
                context.signer()?.as_ref(),
                config.evm_loader,
                &storage_account,
            )
            .await
            .map(|result| json!(result))
        }
        ("neon-elf-params", Some(params)) => {
            let program_location = params.value_of("program_location");
            get_neon_elf::execute(config, context, program_location)
                .await
                .map(|result| json!(result))
        }
        ("collect-treasury", Some(_)) => collect_treasury::execute(config, context)
            .await
            .map(|result| json!(result)),
        ("init-environment", Some(params)) => {
            let file = params.value_of("file");
            let send_trx = params.is_present("send-trx");
            let force = params.is_present("force");
            let keys_dir = params.value_of("keys-dir");
            init_environment::execute(config, context, send_trx, force, keys_dir, file)
                .await
                .map(|result| json!(result))
        }
        ("get-storage-at", Some(params)) => {
            let contract_id = address_of(params, "contract_id").expect("contract_it parse error");
            let index = u256_of(params, "index").expect("index parse error");
            get_storage_at::execute(context.rpc_client, &config.evm_loader, contract_id, &index)
                .await
                .map(|hash| json!(hex::encode(hash.0)))
        }
        _ => unreachable!(),
    }
}

fn parse_tx(params: &ArgMatches) -> (TxParams, TraceCallConfig) {
    let from = address_of(params, "sender").expect("sender parse error");
    let to = address_or_deploy_of(params, "contract");
    let transaction_params: Option<TransactionParams> = read_from_stdin().unwrap_or_else(|err| {
        panic!("Unable to parse `TransactionParams` from STDIN, error: {err:?}")
    });
    let (data, trace_config) = transaction_params
        .map(|params| {
            (
                params.data.map(Into::into),
                params.trace_config.unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    let value = u256_of(params, "value");
    let gas_limit = u256_of(params, "gas_limit");

    let tx_params = TxParams {
        nonce: None,
        from,
        to,
        data,
        value,
        gas_limit,
    };

    (tx_params, trace_config)
}

async fn parse_tx_hash(rpc_client: &dyn Rpc) -> (TxParams, TraceConfig) {
    let tx = rpc_client.get_transaction_data().await.unwrap();
    let transaction_params: Option<TransactionHashParams> =
        read_from_stdin().unwrap_or_else(|err| {
            panic!("Unable to parse `TransactionHashParams` from STDIN, error: {err:?}")
        });

    let trace_config = transaction_params
        .map(|params| params.trace_config.unwrap_or_default())
        .unwrap_or_default();

    (tx, trace_config)
}

pub async fn parse_tx_params<'a>(
    config: &Config,
    context: &Context<'_>,
    params: &'a ArgMatches<'a>,
) -> (Pubkey, u64, u64, Vec<Address>, Vec<Pubkey>) {
    // Read ELF params only if token_mint or chain_id is not set.
    let mut token = pubkey_of(params, "token_mint");
    let mut chain = value_of(params, "chain_id");
    if token.is_none() || chain.is_none() {
        let cached_elf_params = CachedElfParams::new(config, context).await;
        token = token.or_else(|| {
            Some(
                Pubkey::from_str(
                    cached_elf_params
                        .get("NEON_TOKEN_MINT")
                        .expect("NEON_TOKEN_MINT load error"),
                )
                .expect("NEON_TOKEN_MINT Pubkey ctor error "),
            )
        });
        chain = chain.or_else(|| {
            Some(
                u64::from_str(
                    cached_elf_params
                        .get("NEON_CHAIN_ID")
                        .expect("NEON_CHAIN_ID load error"),
                )
                .expect("NEON_CHAIN_ID u64 ctor error"),
            )
        });
    }
    let token = token.expect("token_mint get error");
    let chain = chain.expect("chain_id get error");
    let max_steps =
        value_of::<u64>(params, "max_steps_to_execute").expect("max_steps_to_execute parse error");

    let accounts = values_of::<Address>(params, "cached_accounts").unwrap_or_default();
    let solana_accounts = values_of::<Pubkey>(params, "solana_accounts").unwrap_or_default();

    (token, chain, max_steps, accounts, solana_accounts)
}

fn address_or_deploy_of(matches: &ArgMatches<'_>, name: &str) -> Option<Address> {
    if matches.value_of(name) == Some("deploy") {
        return None;
    }
    address_of(matches, name)
}

fn address_of(matches: &ArgMatches<'_>, name: &str) -> Option<Address> {
    matches
        .value_of(name)
        .map(|value| Address::from_hex(value).unwrap())
}

fn u256_of(matches: &ArgMatches<'_>, name: &str) -> Option<U256> {
    matches.value_of(name).map(|value| {
        if value.is_empty() {
            return U256::ZERO;
        }

        U256::from_str_prefixed(value).unwrap()
    })
}

fn read_from_stdin<T: serde::de::DeserializeOwned>() -> serde_json::Result<Option<T>> {
    let mut stdin = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin)
        .map_err(serde_json::Error::io)?;
    if stdin.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&stdin).map(Some)
}
