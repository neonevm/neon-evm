mod account_storage;
use crate::account_storage::EmulatorAccountStorage;

use evm_loader::solana_backend::SolanaBackend;

use evm::{executor::StackExecutor, ExitReason};
use hex;
use primitive_types::{H160, U256};
use solana_sdk::pubkey::Pubkey;
use std::{
    env, str::FromStr,
    process::exit
};

use clap::{
    crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker}
};

fn emulate(solana_url: String, evm_loader: Pubkey, contract_id: H160, caller_id: H160, data: Vec<u8>) {
    let account_storage = EmulatorAccountStorage::new(solana_url, evm_loader, contract_id, caller_id);

    let (exit_reason, result, applies_logs) = {
        let backend = SolanaBackend::new(&account_storage, None);
        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    
        let (exit_reason, result) = executor.transact_call(caller_id, contract_id, U256::zero(), data, usize::max_value());
    
        eprintln!("Call done");
        
        if exit_reason.is_succeed() {
            eprintln!("Succeed execution");
            let (applies, logs) = executor.deconstruct();
            (exit_reason, result, Some((applies, logs)))
        } else {
            (exit_reason, result, None)
        }
    };

    eprintln!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, _logs) = applies_logs.unwrap();
    
            account_storage.apply(applies);

            eprintln!("Applies done");
            "succeed".to_string()
        }
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
    };

    eprintln!("{}", &status);
    eprintln!("{}", &hex::encode(&result));

    if !exit_reason.is_succeed() {
        eprintln!("Not succeed execution");
    }

    account_storage.get_used_accounts(&status, &result);
}

fn make_clean_hex(in_str: &str) -> String {
    if &in_str[..2] == "0x" {
        in_str[2..].to_string()
    } else {        
        in_str.to_string()
    }
}

// Return H160 for an argument
fn h160_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    matches.value_of(name).map(|value| {
        H160::from_str(&make_clean_hex(value)).unwrap()
    })
}

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    H160::from_str(&make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return hexdata for an argument
fn hexdata_of(matches: &ArgMatches<'_>, name: &str) -> Option<Vec<u8>> {
    matches.value_of(name).map(|value| {
        hex::decode(value).unwrap()
    })
}

// Return an error if string cannot be parsed as a hexdata
fn is_valid_hexdata<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    hex::decode(string.as_ref()).map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .short("u")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .global(true)
                .validator(is_url_or_moniker)
                .default_value("http://localhost:8899")
                .help("URL for Solana node"),
        )
        .arg(
            Arg::with_name("evm_loader")
                .long("evm_loader")
                .value_name("EVM_LOADER")
                .takes_value(true)
                .global(true)
                .validator(is_valid_pubkey)
                .help("Pubkey for evm_loader contract")
        )
        .subcommand(
            SubCommand::with_name("emulate")
                .about("Emulate execution of Ethereum transaction")
                .arg(
                    Arg::with_name("sender")
                        .value_name("SENDER")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .validator(is_valid_h160)
                        .help("The sender of the transaction")
                )
                .arg(
                    Arg::with_name("contract")
                        .value_name("CONTRACT")
                        .takes_value(true)
                        .index(2)
                        .required(true)
                        .validator(is_valid_h160)
                        .help("The contract that executes the transaction")
                )
                .arg(
                    Arg::with_name("data")
                        .value_name("DATA")
                        .takes_value(true)
                        .index(3)
                        .required(true)
                        .validator(is_valid_hexdata)
                        .help("Transaction data")
                )
        )
        .get_matches();

        let (sub_command, sub_matches) = app_matches.subcommand();
        let matches = sub_matches.unwrap();

        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let json_rpc_url = normalize_to_url_if_moniker(
            matches
                .value_of("json_rpc_url")
                .unwrap_or(&cli_config.json_rpc_url),
        );
        let evm_loader = pubkey_of(&app_matches, "evm_loader")
                .unwrap_or_else(|| {
                    eprintln!("Need specify evm_loader");
                    exit(1);
                });
        println!("evm_loader: {:?}", evm_loader);

        match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_of(&arg_matches, "contract").unwrap();
                let sender = h160_of(&arg_matches, "sender").unwrap();
                let data = hexdata_of(&arg_matches, "data").unwrap();
                emulate(json_rpc_url, evm_loader, contract, sender, data);
            }
            _ => unreachable!(),
        }
}
