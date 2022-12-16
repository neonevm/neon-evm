use solana_clap_utils::{input_validators::{is_url_or_moniker, is_valid_pubkey},};
use clap::{crate_description, crate_name, App, AppSettings, Arg, ArgMatches, SubCommand,};
use evm_loader::{H160, U256, H256};
use std::{str::FromStr, fmt::Display,};

pub fn truncate(in_str: &str) -> &str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {
        in_str
    }
}

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    H160::from_str(truncate(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160_or_deploy<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    if string.as_ref() == "deploy" {
        return Ok(());
    }
    H160::from_str(truncate(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return an error if string cannot be parsed as a U256 integer
fn is_valid_u256<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    U256::from_str(truncate(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

fn is_valid_h256<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    H256::from_str(truncate(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

fn is_amount<T, U>(amount: U) -> Result<(), String>
    where
        T: std::str::FromStr,
        U: AsRef<str> + Display,
{
    if amount.as_ref().parse::<T>().is_ok() {
        Ok(())
    } else {
        Err(format!(
            "Unable to parse argument as {}, provided: {}",
            std::any::type_name::<T>(), amount
        ))
    }
}

macro_rules! sender_contract_value {
    ($cmd:expr, $desc:expr) => {
        SubCommand::with_name($cmd)
                .about($desc)
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
                        .validator(is_valid_h160_or_deploy)
                        .help("The contract that executes the transaction or 'deploy'")
                )
                .arg(
                    Arg::with_name("value")
                        .value_name("VALUE")
                        .takes_value(true)
                        .index(3)
                        .required(false)
                        .validator(is_amount::<U256, _>)
                        .help("Transaction value")
                )
    }
}



#[allow(clippy::too_many_lines)]
pub fn parse<'a >() -> ArgMatches<'a> {
    App::new(crate_name!())
        .about(crate_description!())
        .version(concat!("Neon-cli/v", env!("CARGO_PKG_VERSION"), "-", env!("NEON_REVISION")))
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");

            #[allow(clippy::option_if_let_else)]
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("db_config")
                .long("db_config")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use Postgress DB")
        )
        .arg(
            Arg::with_name("slot")
                .long("slot")
                .value_name("SLOT")
                .takes_value(true)
                .required(false)
                .global(true)
                .help("Slot for db-client (only for trace_call command)"),
        )
        .arg(
            Arg::with_name("hash")
                .long("hash")
                .value_name("HASH")
                .takes_value(true)
                .required(false)
                .global(true)
                .help("Transaction hash for db-client (only for trace_tx command)"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .global(true)
                .multiple(true)
                .help("Increase message verbosity"),
        )
        .arg(
            Arg::with_name("fee_payer")
                .long("fee-payer")
                .takes_value(true)
                .global(true)
                .help("Specify fee payer for transactions (use default solana account if not specified)")
        )
        .arg(
            Arg::with_name("keypair")
                .long("keypair")
                .takes_value(true)
                .global(true)
                .help("Specify signer for transactions (use default solana account if not specified)")
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .short("u")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .global(true)
                .validator(is_url_or_moniker)
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
        .arg(
            Arg::with_name("commitment")
                .long("commitment")
                .takes_value(true)
                .possible_values(&[
                    "processed",
                    "confirmed",
                    "finalized",
                    "recent", // Deprecated as of v1.5.5
                    "single", // Deprecated as of v1.5.5
                    "singleGossip", // Deprecated as of v1.5.5
                    "root", // Deprecated as of v1.5.5
                    "max", // Deprecated as of v1.5.5
                ])
                .value_name("COMMITMENT_LEVEL")
                .hide_possible_values(true)
                .global(true)
                .default_value("finalized")
                .help("Return information at the selected commitment level [possible values: processed, confirmed, finalized]"),
        )
        .arg(
            Arg::with_name("logging_ctx")
                .short("L")
                .long("logging_ctx")
                .value_name("LOG_CONTEXT")
                .takes_value(true)
                .global(true)
                .help("Logging context"),
        )
        .arg(
            Arg::with_name("loglevel")
                .short("l")
                .long("loglevel")
                .value_name("LOG_LEVEL")
                .takes_value(true)
                .global(true)
                .help("Logging level"),
        )
        // TODO: remove it
        .arg(
            Arg::with_name("token_mint")
                .long("token_mint")
                .value_name("TOKEN_MINT")
                .takes_value(true)
                .global(true)
                .validator(is_valid_pubkey)
                .help("Pubkey for token_mint")
        )
        .arg(
            Arg::with_name("chain_id")
                .long("chain_id")
                .value_name("CHAIN_ID")
                .takes_value(true)
                .required(false)
                .help("Network chain_id"),
        )
        .arg(
            Arg::with_name("max_steps_to_execute")
                .long("max_steps_to_execute")
                .value_name("NUMBER_OF_STEPS")
                .takes_value(true)
                .required(false)
                .default_value("100000")
                .help("Maximal number of steps to execute in a single run"),
        )

        .subcommand(
            sender_contract_value!("emulate", "Emulate execution of Ethereum transaction")
        )
        .subcommand(
            sender_contract_value!("trace_call", "Getting traces of Ethereum transaction execution")
                .arg(
                    Arg::with_name("gas_limit")
                        .short("G")
                        .long("gas_slimit")
                        .value_name("GAS_LIMIT")
                        .takes_value(true)
                        .required(false) // TODO: check it
                        .validator(is_amount::<U256, _>)
                        .help("Gas limit"),
                )
                .arg(
                    Arg::with_name("slot")
                        .short("L")
                        .long("slot")
                        .value_name("slot")
                        .takes_value(true)
                        .required(true)
                        .validator(is_amount::<u64, _>)
                        .help("Slot number"),
                )
        )
        .subcommand(
            SubCommand::with_name("trace-trx")
                .about("Getting traces of transaction execution by hash")
                .arg(
                    Arg::with_name("hash")
                        .index(1)
                        .value_name("hash")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_h256)
                        .help("Neon transaction hash"),
                )
        )
        .subcommand(
            SubCommand::with_name("create-ether-account")
                .about("Create ethereum account")
                .arg(
                    Arg::with_name("ether")
                        .index(1)
                        .value_name("ether")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_h160)
                        .help("Ethereum address"),
                )
        )
        .subcommand(
            SubCommand::with_name("create-program-address")
                .about("Generate a program address")
                .arg(
                    Arg::with_name("seed")
                        .index(1)
                        .value_name("SEED_STRING")
                        .takes_value(true)
                        .required(true)
                        .help("The seeds (a string containing seeds in hex form, separated by spaces)"),
                )
        )
        .subcommand(
            SubCommand::with_name("deposit")
                .about("Deposit NEONs to ether account")
                .arg(
                    Arg::with_name("amount")
                        .index(1)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .validator(is_amount::<u64, _>)
                        .help("Amount to deposit"),
                )
                .arg(
                    Arg::with_name("ether")
                        .index(2)
                        .value_name("ETHER")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_h160)
                        .help("Ethereum address"),
                )
        )
        .subcommand(
            SubCommand::with_name("get-ether-account-data")
                .about("Get values stored in associated with given address account data")
                .arg(
                    Arg::with_name("ether")
                        .index(1)
                        .value_name("ether")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_h160)
                        .help("Ethereum address"),
                )
        )
        .subcommand(
            SubCommand::with_name("cancel-trx")
                .about("Cancel NEON transaction")
                .arg(
                    Arg::with_name("storage_account")
                        .index(1)
                        .value_name("STORAGE_ACCOUNT")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("storage account for transaction"),
                )
        )
        .subcommand(
            SubCommand::with_name("neon-elf-params")
                .about("Get NEON values stored in elf")
                .arg(
                    Arg::with_name("program_location")
                        .index(1)
                        .value_name("PROGRAM_FILEPATH")
                        .takes_value(true)
                        .required(false)
                        .help("/path/to/evm_loader.so"),
                )
        )
        .subcommand(
            SubCommand::with_name("collect-treasury")
                .about("Collect lamports from auxiliary treasury accounts to the main treasury balance")
        )
        .subcommand(
            SubCommand::with_name("init-environment")
                .about("Initialize and verify environment for NeonEVM execution")
                .arg(
                    Arg::with_name("send-trx")
                        .long("send-trx")
                        .takes_value(false)
                        .help("Send transaction for initialize"),
                )
                .arg(
                    Arg::with_name("force")
                        .long("force")
                        .takes_value(false)
                        .help("Force initialize environment (even if NeonEVM and CLI version mismatch)"),
                )
                .arg(
                    Arg::with_name("keys-dir")
                        .long("keys-dir")
                        .takes_value(true)
                        .help("Directory with private-keys")
                )
                .arg(
                    Arg::with_name("file")
                        .index(1)
                        .value_name("FILE")
                        .takes_value(true)
                        .required(false)
                        .help("Path to file with program image /path/to/evm_loader.so"),
                )
        )
        .subcommand(
            SubCommand::with_name("get-storage-at")
                .about("Get Ethereum storage value at given index")
                .arg(
                    Arg::with_name("contract_id")
                        .index(1)
                        .value_name("contract_id")
                        .takes_value(true)
                        .validator(is_valid_h160)
                        .required(true),
                )
                .arg(
                    Arg::with_name("index")
                        .index(2)
                        .value_name("index")
                        .takes_value(true)
                        .validator(is_valid_u256)
                        .required(true),
                )
        )
        .get_matches()
}
