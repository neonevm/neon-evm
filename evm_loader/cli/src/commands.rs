pub use neon_lib::commands::*;
use neon_lib::{context::truncate, NeonError};
use serde_json::json;

use crate::{commands::get_neon_elf::CachedElfParams, context::Context, types::TxParams, Config};
use clap::ArgMatches;
use ethnum::U256;
use evm_loader::types::Address;
use solana_clap_utils::input_parsers::{pubkey_of, value_of, values_of};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub type NeonCliResult = Result<serde_json::Value, NeonError>;

#[allow(clippy::too_many_lines)]
pub fn execute(
    cmd: &str,
    params: Option<&ArgMatches>,
    config: &Config,
    context: &Context,
) -> NeonCliResult {
    Ok(match (cmd, params) {
        ("emulate", Some(params)) => {
            let tx = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params);
            json!(emulate::execute(
                config,
                context,
                tx,
                token,
                chain,
                steps,
                &accounts,
                &solana_accounts,
            )?)
        }
        ("emulate_hash", Some(params)) => {
            let tx = context.rpc_client.get_transaction_data()?;
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params);
            json!(emulate::execute(
                config,
                context,
                tx,
                token,
                chain,
                steps,
                &accounts,
                &solana_accounts,
            )?)
        }
        ("trace", Some(params)) => {
            let tx = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params);
            json!(trace::execute(
                config,
                context,
                tx,
                token,
                chain,
                steps,
                &accounts,
                &solana_accounts,
            )?)
        }
        ("trace_hash", Some(params)) => {
            let tx = context.rpc_client.get_transaction_data()?;
            let (token, chain, steps, accounts, solana_accounts) =
                parse_tx_params(config, context, params);
            json!(trace::execute(
                config,
                context,
                tx,
                token,
                chain,
                steps,
                &accounts,
                &solana_accounts,
            )?)
        }
        ("create-ether-account", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            json!(create_ether_account::execute(config, context, &ether)?)
        }
        ("deposit", Some(params)) => {
            let amount = value_of(params, "amount").expect("amount parse error");
            let ether = address_of(params, "ether").expect("ether parse error");
            json!(deposit::execute(config, context, amount, &ether)?)
        }
        ("get-ether-account-data", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            json!(get_ether_account_data::execute(config, context, &ether)?)
        }
        ("cancel-trx", Some(params)) => {
            let storage_account =
                pubkey_of(params, "storage_account").expect("storage_account parse error");
            json!(cancel_trx::execute(config, context, &storage_account)?)
        }
        ("neon-elf-params", Some(params)) => {
            let program_location = params.value_of("program_location");
            json!(get_neon_elf::execute(config, context, program_location)?)
        }
        ("collect-treasury", Some(_)) => {
            json!(collect_treasury::execute(config, context)?)
        }
        ("init-environment", Some(params)) => {
            let file = params.value_of("file");
            let send_trx = params.is_present("send-trx");
            let force = params.is_present("force");
            let keys_dir = params.value_of("keys-dir");
            json!(init_environment::execute(
                config, context, send_trx, force, keys_dir, file,
            )?)
        }
        ("get-storage-at", Some(params)) => {
            let contract_id = address_of(params, "contract_id").expect("contract_it parse error");
            let index = u256_of(params, "index").expect("index parse error");
            json!(get_storage_at::execute(
                config,
                context,
                contract_id,
                &index,
            )?)
        }
        _ => unreachable!(),
    })
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

fn read_stdin() -> Option<Vec<u8>> {
    let mut data = String::new();

    if let Ok(len) = std::io::stdin().read_line(&mut data) {
        if len == 0 {
            return None;
        }
        let data = truncate(data.as_str());
        let bin = hex::decode(data).expect("data hex::decore error");
        Some(bin)
    } else {
        None
    }
}

fn parse_tx(params: &ArgMatches) -> TxParams {
    let from = address_of(params, "sender").expect("sender parse error");
    let to = address_or_deploy_of(params, "contract");
    let data = read_stdin();
    let value = u256_of(params, "value");
    let gas_limit = u256_of(params, "gas_limit");

    TxParams {
        from,
        to,
        data,
        value,
        gas_limit,
    }
}

pub fn parse_tx_params(
    config: &Config,
    context: &Context,
    params: &ArgMatches,
) -> (Pubkey, u64, u64, Vec<Address>, Vec<Pubkey>) {
    // Read ELF params only if token_mint or chain_id is not set.
    let mut token = pubkey_of(params, "token_mint");
    let mut chain = value_of(params, "chain_id");
    if token.is_none() || chain.is_none() {
        let cached_elf_params = CachedElfParams::new(config, context);
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
