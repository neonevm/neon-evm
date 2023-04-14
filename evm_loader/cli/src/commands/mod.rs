pub mod cancel_trx;
pub mod collect_treasury;
pub mod create_ether_account;
pub mod deposit;
pub mod emulate;
pub mod get_ether_account_data;
pub mod get_neon_elf;
pub mod get_storage_at;
pub mod init_environment;
mod trace;
mod transaction_executor;

use crate::{
    commands::get_neon_elf::CachedElfParams, program_options::truncate, types::TxParams, Config,
    NeonCliResult,
};
use clap::ArgMatches;
use ethnum::U256;
use evm_loader::types::Address;
use solana_clap_utils::input_parsers::{pubkey_of, value_of, values_of};
use solana_client::{
    client_error::Result as SolanaClientResult, rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use std::str::FromStr;

pub fn execute(cmd: &str, params: Option<&ArgMatches>, config: &Config) -> NeonCliResult {
    match (cmd, params) {
        ("emulate", Some(params)) => {
            let tx = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) = parse_tx_params(config, params);
            emulate::execute(config, tx, token, chain, steps, &accounts, &solana_accounts)
        }
        ("emulate_hash", Some(params)) => {
            let tx = config.rpc_client.get_transaction_data()?;
            let (token, chain, steps, accounts, solana_accounts) = parse_tx_params(config, params);
            emulate::execute(config, tx, token, chain, steps, &accounts, &solana_accounts)
        }
        ("trace", Some(params)) => {
            let tx = parse_tx(params);
            let (token, chain, steps, accounts, solana_accounts) = parse_tx_params(config, params);
            trace::execute(config, tx, token, chain, steps, &accounts, &solana_accounts)
        }
        ("trace_hash", Some(params)) => {
            let tx = config.rpc_client.get_transaction_data()?;
            let (token, chain, steps, accounts, solana_accounts) = parse_tx_params(config, params);
            trace::execute(config, tx, token, chain, steps, &accounts, &solana_accounts)
        }
        ("create-ether-account", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            create_ether_account::execute(config, &ether)
        }
        ("deposit", Some(params)) => {
            let amount = value_of(params, "amount").expect("amount parse error");
            let ether = address_of(params, "ether").expect("ether parse error");
            deposit::execute(config, amount, &ether)
        }
        ("get-ether-account-data", Some(params)) => {
            let ether = address_of(params, "ether").expect("ether parse error");
            get_ether_account_data::execute(config, &ether)
        }
        ("cancel-trx", Some(params)) => {
            let storage_account =
                pubkey_of(params, "storage_account").expect("storage_account parse error");
            cancel_trx::execute(config, &storage_account)
        }
        ("neon-elf-params", Some(params)) => {
            let program_location = params.value_of("program_location");
            get_neon_elf::execute(config, program_location)
        }
        ("collect-treasury", Some(_)) => collect_treasury::execute(config),
        ("init-environment", Some(params)) => {
            let file = params.value_of("file");
            let send_trx = params.is_present("send-trx");
            let force = params.is_present("force");
            let keys_dir = params.value_of("keys-dir");
            init_environment::execute(config, send_trx, force, keys_dir, file)
        }
        ("get-storage-at", Some(params)) => {
            let contract_id = address_of(params, "contract_id").expect("contract_it parse error");
            let index = u256_of(params, "index").expect("index parse error");
            get_storage_at::execute(config, contract_id, &index)
        }
        _ => unreachable!(),
    }
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

pub fn send_transaction(
    config: &Config,
    instructions: &[Instruction],
) -> SolanaClientResult<Signature> {
    let message = Message::new(instructions, Some(&config.signer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    let signers = [&*config.signer];
    let (blockhash, _last_valid_slot) = config
        .rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())?;
    transaction.try_sign(&signers, blockhash)?;

    config
        .rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &transaction,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                ..RpcSendTransactionConfig::default()
            },
        )
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
    params: &ArgMatches,
) -> (Pubkey, u64, u64, Vec<Address>, Vec<Pubkey>) {
    // Read ELF params only if token_mint or chain_id is not set.
    let mut token = pubkey_of(params, "token_mint");
    let mut chain = value_of(params, "chain_id");
    if token.is_none() || chain.is_none() {
        let cached_elf_params = CachedElfParams::new(config);
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
