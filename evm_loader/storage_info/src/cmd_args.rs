
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


pub fn parse_program_args() -> (Pubkey, String, Pubkey) {
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
        )
        .arg(
            Arg::with_name("operator")
                .value_name("operator")
                .takes_value(true)
                .required(true)
                .global(true)
                .validator(is_valid_pubkey)
                .help("Operator's pubkey")
        )
        .get_matches();

    let evm_loader = pubkey_of(&app_matches, "evm_loader")
        .unwrap_or_else(|| {
            println!("Need to specify evm_loader");
            exit(1);
        });
    println!("evm_loader:   {:?}", evm_loader);

    let operator = pubkey_of(&app_matches, "operator")
        .unwrap_or_else(|| {
            println!("Need to specify operator");
            exit(1);
        });
    println!("operator:   {:?}", operator);


    let json_rpc_url = normalize_to_url_if_moniker(
        app_matches
            .value_of("json_rpc_url").unwrap()
    );
    println!("url:   {:?}", json_rpc_url);



    return (evm_loader, json_rpc_url, operator);
}
