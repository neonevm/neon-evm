mod account_storage;
use crate::account_storage::EmulatorAccountStorage;

use evm_loader::{
    instruction::EvmInstruction,
    solana_backend::SolanaBackend,
};

use evm::{executor::StackExecutor, ExitReason};
use hex;
use primitive_types::{H160, U256};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Signer,
    transaction::Transaction,
    system_program,
};
use serde_json::json;
use std::{
    env, str::FromStr,
    process::exit
};

use clap::{
    crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_clap_utils::{
    input_parsers::{pubkey_of, value_of},
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker},
    keypair::{signer_from_path},
};

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use solana_cli::{
    checks::check_account_for_fee,
};

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

pub struct Config {
    rpc_client: RpcClient,
    verbose: bool,
    evm_loader: Pubkey,
    fee_payer: Pubkey,
    signer: Box<dyn Signer>,
}

fn command_emulate(config: &Config, contract_id: H160, caller_id: H160, data: Vec<u8>) -> CommandResult {
    let account_storage = EmulatorAccountStorage::new(config, contract_id, caller_id);

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

    Ok(())
}

fn command_create_program_address (
    config: &Config,
    seed: &str,
) -> CommandResult {
    let strings = seed.split_whitespace().collect::<Vec<_>>();
    let mut seeds = vec![];
    let mut seeds_vec = vec![];
    for s in strings {
        seeds_vec.push(hex::decode(s).unwrap());
    }
    for i in &seeds_vec {seeds.push(&i[..]);}
    let (address,nonce) = Pubkey::find_program_address(&seeds, &config.evm_loader);
    println!("{} {}", address, nonce);

    Ok(())
}

fn command_create_ether_account (
    config: &Config,
    ether_address: &H160,
    lamports: u64,
    space: u64
) -> CommandResult {
    let (solana_address, nonce) = Pubkey::find_program_address(&[ether_address.as_bytes()], &config.evm_loader);
    println!("Create ethereum account {} <- {} {}", solana_address, hex::encode(ether_address), nonce);

    let instruction = Instruction::new(
            config.evm_loader,
            &EvmInstruction::CreateAccount {lamports, space, ether: *ether_address, nonce},
            vec![
                AccountMeta::new(config.signer.pubkey(), true),
                AccountMeta::new(solana_address, false),
                AccountMeta::new_readonly(system_program::id(), false)
            ]);

    let finalize_message = Message::new(&[instruction], Some(&config.signer.pubkey()));
    let (blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &fee_calculator,
        &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    println!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    println!("{}", json!({
        "solana": format!("{}", solana_address),
        "ether": format!("{}", hex::encode(ether_address)),
        "nonce": nonce,
    }).to_string());
    Ok(())
}

fn make_clean_hex<'a>(in_str: &'a str) -> &'a str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {        
        &in_str
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
        hex::decode(&make_clean_hex(value)).unwrap()
    })
}

// Return an error if string cannot be parsed as a hexdata
fn is_valid_hexdata<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    hex::decode(&make_clean_hex(string.as_ref())).map(|_| ())
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
                .arg(
                    Arg::with_name("lamports")
                        .long("lamports")
                        .value_name("lamports")
                        .takes_value(true)
                        .default_value("0")
                        .required(false)
                )
                .arg(
                    Arg::with_name("space")
                        .long("space")
                        .value_name("space")
                        .takes_value(true)
                        .required(false)
                        .default_value("0")
                        .help("Length of data for new account"),
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
            SubCommand::with_name("deploy")
                .about("Deploy a program")
                .arg(
                    Arg::with_name("program_location")
                        .index(1)
                        .value_name("PROGRAM_FILEPATH")
                        .takes_value(true)
                        .required(true)
                        .help("/path/to/program.o"),
                )
                /*.arg(
                    Arg::with_name("address_signer")
                        .index(2)
                        .value_name("PROGRAM_ADDRESS_SIGNER")
                        .takes_value(true)
                        .validator(is_valid_signer)
                        .help("The signer for the desired address of the program [default: new random address]")
                )*/
                .arg(
                    Arg::with_name("allow_excessive_balance")
                        .long("allow-excessive-deploy-account-balance")
                        .takes_value(false)
                        .help("Use the designated program id, even if the account already holds a large balance of SOL")
                )
                //.arg(commitment_arg_with_default("max")),
        )
        .get_matches();

        let mut wallet_manager = None;
        let config = {
            let cli_config = if let Some(config_file) = app_matches.value_of("config_file") {
                solana_cli_config::Config::load(config_file).unwrap_or_default()
            } else {
                solana_cli_config::Config::default()
            };

            let json_rpc_url = normalize_to_url_if_moniker(
                app_matches
                    .value_of("json_rpc_url")
                    .unwrap_or(&cli_config.json_rpc_url),
            );

            let evm_loader = pubkey_of(&app_matches, "evm_loader")
                    .unwrap_or_else(|| {
                        eprintln!("Need specify evm_loader");
                        exit(1);
                    });

            let verbose = app_matches.is_present("verbose");

            let (signer, fee_payer) = signer_from_path(
                &app_matches,
                app_matches
                    .value_of("fee_payer")
                    .unwrap_or(&cli_config.keypair_path),
                "fee_payer",
                &mut wallet_manager,
            )
            .map(|s| {
                let p = s.pubkey();
                (s, p)
            })
            .unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                exit(1);
            });

            Config {
                rpc_client: RpcClient::new_with_commitment(json_rpc_url, CommitmentConfig::recent()),
                verbose,
                evm_loader,
                fee_payer,
                signer,
            }
        };

        let (sub_command, sub_matches) = app_matches.subcommand();
        let result = match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_of(&arg_matches, "contract").unwrap();
                let sender = h160_of(&arg_matches, "sender").unwrap();
                let data = hexdata_of(&arg_matches, "data").unwrap();

                command_emulate(&config, contract, sender, data)
            }
            ("create-program-address", Some(arg_matches)) => {
                let seed = arg_matches.value_of("seed").unwrap().to_string();

                command_create_program_address(&config, &seed)
            }
            ("create-ether-account", Some(arg_matches)) => {
                //let signers = vec![default_signer.signer_from_path(arg_matches, wallet_manager)?];
                let ether = h160_of(&arg_matches, "ether").unwrap();
                let lamports = value_t_or_exit!(arg_matches, "lamports", u64);
                let space = value_t_or_exit!(arg_matches, "space", u64);

                command_create_ether_account(&config, &ether, lamports, space)
            }
            _ => unreachable!(),
        };
        match result {
            Ok(()) => exit(0),
            Err(err) => {
                eprintln!("error: {}", err);
                exit(1);
            }
        }
}
