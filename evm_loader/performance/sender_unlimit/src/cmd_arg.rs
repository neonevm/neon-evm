
use solana_sdk::{
    pubkey::Pubkey,
};

use std::{
    process::exit,
    env
};

use clap::{
    crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker},
    keypair::{signer_from_path, keypair_from_path},
};


pub fn parse_program_args() -> (Pubkey, String, String, String, String, String, String, u64) {
    let key = "EVM_LOADER";
    let env_evm_loader  = match env::var_os(key) {
        Some(val) => val.into_string().unwrap(),
        None => "".to_string()
    };

    let key = "SOLANA_URL";
    let env_solana_url  = match env::var_os(key) {
        Some(val) => val.into_string().unwrap(),
        None => "http://localhost:8899".to_string()
    };

    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        // .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("json_rpc_url")
                .short("u")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .global(true)
                .validator(is_url_or_moniker)
                .default_value(&*env_solana_url)
                .help("URL for Solana node"),
        )
        .arg(
            Arg::with_name("evm_loader")
                .long("evm_loader")
                .value_name("EVM_LOADER")
                .takes_value(true)
                .global(true)
                .validator(is_valid_pubkey)
                .default_value(&*env_evm_loader)
                .help("Pubkey for evm_loader contract")
        ).arg(
        Arg::with_name("sender_file")
            .value_name("SENDER_FILEPATH")
            .takes_value(true)
            .required(true)
            .help("/path/to/sender.json")
            .default_value("sender.json"),
        )
        .arg(
            Arg::with_name("verify_file")
                .value_name("VERIFY_FILEPATH")
                .takes_value(true)
                .required(true)
                .help("/path/to/verify.json")
                .default_value("verify.json"),
        )
        .arg(
            Arg::with_name("collateral_file")
                .value_name("COLLATERAL_FILEPATH")
                .takes_value(true)
                .required(true)
                .help("/path/to/collateral.json")
                .default_value("collateral.json"),
        )
        .arg(
            Arg::with_name("account_file")
                .value_name("ACCOUNT_FILEPATH")
                .takes_value(true)
                .required(true)
                .help("/path/to/account.json")
                .default_value("account.json"),
        )
        .arg(
            Arg::with_name("client")
                .long("client")
                .value_name("CLIENT")
                .takes_value(true)
                .global(true)
                .help("tcp, udp")
                .possible_values(&["tcp", "udp"])
                .default_value("udp"),
        )
        .arg(
            Arg::with_name("delay")
                .long("delay")
                .value_name("DELAY")
                .takes_value(true)
                .global(true)
                .help("delay in microseconds between sending trx")
                .default_value("1000"),
        )
        .get_matches();

    let evm_loader = pubkey_of(&app_matches, "evm_loader")
        .unwrap_or_else(|| {
            println!("Need to specify evm_loader");
            exit(1);
        });
    println!("evm_loader:   {:?}", evm_loader);


    let json_rpc_url = normalize_to_url_if_moniker(
        app_matches
            .value_of("json_rpc_url").unwrap()
    );
    println!("url:   {:?}", json_rpc_url);


    let client = app_matches.value_of("client").unwrap().to_string();

    let senders_filename = app_matches.value_of("sender_file").unwrap().to_string();
    let verify_filename = app_matches.value_of("verify_file").unwrap().to_string();
    let collateral_filename = app_matches.value_of("collateral_file").unwrap().to_string();
    let account_filename = app_matches.value_of("account_file").unwrap().to_string();
    let delay :u64 = app_matches.value_of("delay").unwrap().to_string().parse().unwrap();

    return (evm_loader, json_rpc_url, senders_filename, verify_filename, collateral_filename, account_filename, client, delay);
}
