pub mod cancel_trx;
pub mod create_ether_account;
pub mod create_program_address;
pub mod deposit;
pub mod emulate;
pub mod get_ether_account_data;
pub mod get_neon_elf;
pub mod get_storage_at;
pub mod collect_treasury;
pub mod init_environment;
mod transaction_executor;
mod trace_call;

use clap::ArgMatches;
use solana_clap_utils::input_parsers::{pubkey_of, value_of,};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use solana_client::{rpc_config::RpcSendTransactionConfig,client_error::Result as SolanaClientResult,};
use evm_loader::{H160, H256, U256};
use std::str::FromStr;
use evm_loader::account::EthereumAccount;
use log::debug;
use crate::{
    NeonCliResult, NeonCliError,
    program_options::make_clean_hex,
    Config,
    account_storage::account_info,
};

pub fn execute(cmd: &str, params: Option<&ArgMatches>, config: &Config) -> NeonCliResult{
    match (cmd, params) {
        ("emulate", Some(params)) => {
            emulate::execute(config, params,)
        }
        ("create-program-address", Some(params)) => {
            let ether = h160_of(params, "seed").unwrap();
            create_program_address::execute(config, &ether);
            Ok(())
        }
        ("create-ether-account", Some(params)) => {
            let ether = h160_of(params, "ether").unwrap();
            create_ether_account::execute(config, &ether)
        }
        ("deposit", Some(params)) => {
            let amount = value_of(params, "amount").unwrap();
            let ether = h160_of(params, "ether").unwrap();
            deposit::execute(config, amount, &ether)
        }
        ("get-ether-account-data", Some(params)) => {
            let ether = h160_of(params, "ether").unwrap();
            get_ether_account_data::execute(config, &ether);
            Ok(())
        }
        ("cancel-trx", Some(params)) => {
            let storage_account = pubkey_of(params, "storage_account").unwrap();
            cancel_trx::execute(config, &storage_account)
        }
        ("neon-elf-params", Some(params)) => {
            let program_location = params.value_of("program_location");
            get_neon_elf::execute(config, program_location)
        }
        ("collect-treasury", Some(_)) => {
            collect_treasury::execute(config)
        }
        ("init-environment", Some(params)) => {
            let file = params.value_of("file");
            let send_trx = params.is_present("send-trx");
            let force = params.is_present("force");
            let keys_dir = params.value_of("keys-dir");
            init_environment::execute(config, send_trx, force, keys_dir, file)
        }
        ("get-storage-at", Some(params)) => {
            let contract_id = h160_of(params, "contract_id").unwrap();
            let index = u256_of(params, "index").unwrap();
            get_storage_at::execute(config, contract_id, &index);
            Ok(())
        }
        ("trace_call", Some(params)) => {
            trace_call::execute(config, params)
        }
        _ => unreachable!(),
    }
}

fn h160_or_deploy_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    if matches.value_of(name) == Some("deploy") {
        return None;
    }
    matches.value_of(name).map(|value| {
        H160::from_str(make_clean_hex(value)).unwrap()
    })
}

fn h160_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    matches.value_of(name).map(|value| {
        H160::from_str(make_clean_hex(value)).unwrap()
    })
}

fn u256_of(matches: &ArgMatches<'_>, name: &str) -> Option<U256> {
    matches.value_of(name).map(|value| {
        U256::from_str(make_clean_hex(value)).unwrap()
    })
}

fn read_stdin() -> Option<Vec<u8>>{
    let mut data = String::new();

    if let Ok(len) = std::io::stdin().read_line(&mut data){
        if len == 0{
            return None
        }
        let data = make_clean_hex(data.as_str());
        let bin = hex::decode(data).unwrap();
        Some(bin)
    }
    else{
        None
    }
}

fn get_program_ether(
    caller_ether: &H160,
    trx_count: u64
) -> H160 {
    let trx_count_256 : U256 = U256::from(trx_count);
    let mut stream = rlp::RlpStream::new_list(2);
    stream.append(caller_ether);
    stream.append(&trx_count_256);

    let hash = solana_sdk::keccak::hash(&stream.out()).to_bytes();
    H256::from(hash).into()
}

fn get_ether_account_nonce(
    config: &Config,
    caller_sol: &Pubkey,
) -> Result<(u64, H160), NeonCliError> {
    let mut acc = match config.rpc_client.get_account_with_commitment(caller_sol, CommitmentConfig::confirmed())?.value {
        Some(acc) => acc,
        None => return Ok((u64::default(), H160::default()))
    };

    debug!("get_ether_account_nonce account = {:?}", acc);

    let info = account_info(caller_sol, &mut acc);
    let account = EthereumAccount::from_account(&config.evm_loader, &info).map_err(NeonCliError::ProgramError)?;
    let trx_count = account.trx_count;
    let caller_ether = account.address;

    debug!("Caller: ether {}, solana {}", caller_ether, caller_sol);
    debug!("Caller trx_count: {} ", trx_count);

    Ok((trx_count, caller_ether))
}

pub fn send_transaction(
    config: &Config,
    instructions: &[Instruction]
) -> SolanaClientResult<Signature> {
    let message = Message::new(instructions, Some(&config.signer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    let signers = [&*config.signer];
    let (blockhash, _last_valid_slot) = config.rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())?;
    transaction.try_sign(&signers, blockhash)?;

    config.rpc_client.send_and_confirm_transaction_with_spinner_and_config(
        &transaction,
        CommitmentConfig::confirmed(),
        RpcSendTransactionConfig {
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            ..RpcSendTransactionConfig::default()
        },
    )
}

