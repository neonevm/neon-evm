#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]

#[allow(clippy::module_name_repetitions)]
mod build_info;
mod config;
mod logs;
mod program_options;

use neon_lib::{
    commands::{
        cancel_trx, collect_treasury, emulate, get_balance, get_config, get_contract, get_holder,
        get_neon_elf, get_storage_at, init_environment, trace,
    },
    rpc::CloneRpcClient,
    types::{BalanceAddress, EmulateRequest},
    Config,
};

use clap::ArgMatches;
use std::io::Read;

use ethnum::U256;
use log::debug;
use serde_json::json;
use solana_clap_utils::input_parsers::{pubkey_of, value_of};
use tokio::time::Instant;

use crate::build_info::get_build_info;
use evm_loader::types::Address;
use neon_lib::errors::NeonError;
use neon_lib::rpc::{CallDbClient, RpcEnum};
use neon_lib::tracing::tracers::TracerTypeEnum;
use neon_lib::types::TracerDb;
use solana_clap_utils::keypair::signer_from_path;
use solana_sdk::signature::Signer;

type NeonCliResult = Result<serde_json::Value, NeonError>;

#[allow(clippy::too_many_lines)]
async fn run(options: &ArgMatches<'_>) -> NeonCliResult {
    let config = &config::create(options)?;

    match options.subcommand() {
        ("emulate", Some(_)) => {
            let rpc = build_rpc(options, config).await?;

            let request = read_tx_from_stdin()?;
            emulate::execute(&rpc, config.evm_loader, request, None::<TracerTypeEnum>)
                .await
                .map(|(result, _)| json!(result))
        }
        ("trace", Some(_)) => {
            let rpc = build_rpc(options, config).await?;

            let request = read_tx_from_stdin()?;
            trace::trace_transaction(&rpc, config.evm_loader, request)
                .await
                .map(|trace| json!(trace))
        }
        ("get-ether-account-data", Some(params)) => {
            let rpc = build_rpc(options, config).await?;

            let address = address_of(params, "ether").unwrap();
            let chain_id = value_of(params, "chain_id").unwrap();

            let account = BalanceAddress { address, chain_id };
            let accounts = std::slice::from_ref(&account);

            get_balance::execute(&rpc, &config.evm_loader, accounts)
                .await
                .map(|result| json!(result))
        }
        ("get-contract-account-data", Some(params)) => {
            let rpc = build_rpc(options, config).await?;

            let account = address_of(params, "address").unwrap();
            let accounts = std::slice::from_ref(&account);

            get_contract::execute(&rpc, &config.evm_loader, accounts)
                .await
                .map(|result| json!(result))
        }
        ("get-holder-account-data", Some(params)) => {
            let rpc = build_rpc(options, config).await?;

            let account = pubkey_of(params, "account").unwrap();

            get_holder::execute(&rpc, &config.evm_loader, account)
                .await
                .map(|result| json!(result))
        }
        ("cancel-trx", Some(params)) => {
            let rpc_client = CloneRpcClient::new_from_config(config);
            let signer = build_signer(config)?;

            let storage_account =
                pubkey_of(params, "storage_account").expect("storage_account parse error");
            cancel_trx::execute(rpc_client, &*signer, config.evm_loader, &storage_account)
                .await
                .map(|result| json!(result))
        }
        ("neon-elf-params", Some(params)) => {
            let rpc = build_rpc(options, config).await?;

            let program_location = params.value_of("program_location");
            get_neon_elf::execute(config, &rpc, program_location)
                .await
                .map(|result| json!(result))
        }
        ("collect-treasury", Some(_)) => {
            let rpc_client = CloneRpcClient::new_from_config(config);
            let signer = build_signer(config)?;

            collect_treasury::execute(config, &rpc_client, &*signer)
                .await
                .map(|result| json!(result))
        }
        ("init-environment", Some(params)) => {
            let rpc_client = CloneRpcClient::new_from_config(config);
            let signer = build_signer(config)?;

            let file = params.value_of("file");
            let send_trx = params.is_present("send-trx");
            let force = params.is_present("force");
            let keys_dir = params.value_of("keys-dir");

            init_environment::execute(
                config,
                &rpc_client,
                &*signer,
                send_trx,
                force,
                keys_dir,
                file,
            )
            .await
            .map(|result| json!(result))
        }
        ("get-storage-at", Some(params)) => {
            let rpc = build_rpc(options, config).await?;

            let contract_id = address_of(params, "contract_id").expect("contract_it parse error");
            let index = u256_of(params, "index").expect("index parse error");

            get_storage_at::execute(&rpc, &config.evm_loader, contract_id, index)
                .await
                .map(|hash| json!(hex::encode(hash.0)))
        }
        ("config", Some(_)) => {
            let rpc = build_rpc(options, config).await?;

            get_config::execute(&rpc, config.evm_loader)
                .await
                .map(|result| json!(result))
        }
        _ => unreachable!(),
    }
}

async fn build_rpc(options: &ArgMatches<'_>, config: &Config) -> Result<RpcEnum, NeonError> {
    let slot: Option<u64> = options
        .value_of("slot")
        .map(|slot_str| slot_str.parse().expect("slot parse error"));

    Ok(if let Some(slot) = slot {
        RpcEnum::CallDbClient(
            CallDbClient::new(
                TracerDb::new(config.db_config.as_ref().expect("db-config not found")),
                slot,
                None,
            )
            .await?,
        )
    } else {
        RpcEnum::CloneRpcClient(CloneRpcClient::new_from_config(config))
    })
}

fn print_result(result: &NeonCliResult) {
    let logs = {
        let context = logs::CONTEXT.lock().unwrap();
        context.clone()
    };

    let result = match result {
        Ok(value) => json!({
            "result": "success",
            "value": value,
            "logs": logs
        }),
        Err(e) => json!({
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

    debug!("{}", get_build_info());

    let result = run(&options).await;

    let execution_time = Instant::now().duration_since(time_start);
    log::info!("execution time: {} sec", execution_time.as_secs_f64());
    print_result(&result);
    if let Err(e) = result {
        std::process::exit(e.error_code());
    };
}

fn read_tx_from_stdin() -> Result<EmulateRequest, NeonError> {
    let mut stdin_buffer = String::new();
    std::io::stdin().read_to_string(&mut stdin_buffer)?;

    serde_json::from_str(&stdin_buffer).map_err(NeonError::from)
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

/// # Errors
fn build_signer(config: &Config) -> Result<Box<dyn Signer>, NeonError> {
    let mut wallet_manager = None;

    let signer = signer_from_path(
        &ArgMatches::default(),
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonError::KeypairNotSpecified)?;

    Ok(signer)
}
