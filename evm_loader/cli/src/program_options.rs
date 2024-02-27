use clap::{crate_description, crate_name, App, AppSettings, Arg, ArgMatches, SubCommand};
use ethnum::U256;
use evm_loader::types::Address;
use solana_clap_utils::input_validators::{is_url_or_moniker, is_valid_pubkey};
use std::fmt::Display;

// Return an error if string cannot be parsed as a Address address
fn is_valid_address<T>(string: T) -> Result<(), String>
where
    T: AsRef<str>,
{
    Address::from_hex(string.as_ref())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// Return an error if string cannot be parsed as a U256 integer
fn is_valid_u256<T>(string: T) -> Result<(), String>
where
    T: AsRef<str>,
{
    let value = string.as_ref();
    if value.is_empty() {
        return Ok(());
    }

    U256::from_str_prefixed(value)
        .map(|_| ())
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
            "Unable to parse argument as {}, provided: {amount}",
            std::any::type_name::<T>()
        ))
    }
}

#[allow(clippy::too_many_lines)]
pub fn parse<'a>() -> ArgMatches<'a> {
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
                .help("Configuration file to use Tracer DB")
        )
        .arg(
            Arg::with_name("slot")
                .short("L")
                .long("slot")
                .value_name("slot")
                .takes_value(true)
                .required(false)
                .validator(is_amount::<u64, _>)
                .help("Slot number to work with archived data"),
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
            Arg::with_name("solana_key_for_config")
                .long("solana_key_for_config")
                .value_name("SOLANA_KEY_FOR_CONFIG")
                .takes_value(true)
                .global(true)
                .required(false)
                .validator(is_valid_pubkey)
                .help("Operator pubkey, used for config simulation")
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
            Arg::with_name("loglevel")
                .short("l")
                .long("loglevel")
                .value_name("LOG_LEVEL")
                .takes_value(true)
                .global(true)
                .help("Logging level"),
        )
        .subcommand(
            SubCommand::with_name("emulate")
            .about("Emulation transaction. Parameters can be provided via STDIN as a JSON object.")
        )
        .subcommand(
            SubCommand::with_name("trace")
            .about("Emulation transaction to collecting traces. Parameters can be provided via STDIN as a JSON object.")
        )
        .subcommand(
            SubCommand::with_name("get-ether-account-data")
                .alias("balance")
                .about("Get values stored in associated with given address account data")
                .arg(
                    Arg::with_name("ether")
                        .value_name("ETHER")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .validator(is_valid_address)
                        .help("Ethereum address")
                )
                .arg(
                    Arg::with_name("chain_id")
                    .long("chain_id")
                    .value_name("CHAIN_ID")
                    .takes_value(true)
                    .index(2)
                    .required(true)
                    .help("Network chain_id")
                )
        )
        .subcommand(
            SubCommand::with_name("get-contract-account-data")
                .alias("contract")
                .about("Get values stored in associated with given contract")
                .arg(
                    Arg::with_name("address")
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .index(1)
                        .required(true)
                        .validator(is_valid_address)
                        .help("Ethereum address")
                )
        )
        .subcommand(
            SubCommand::with_name("get-holder-account-data")
                .alias("holder")
                .about("Get values stored in a Holder acount")
                .arg(
                    Arg::with_name("account")
                        .index(1)
                        .value_name("ACCOUNT")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("Public Key"),
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
            SubCommand::with_name("config")
                .about("Read configuration parameters from NeonEVM program.")
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
                        .validator(is_valid_address)
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
