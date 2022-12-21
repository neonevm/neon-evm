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
use solana_client::{
    rpc_config::RpcSendTransactionConfig
    client_error::Result as SolanaClientResult, ClientErrorKind
};
use evm_loader::{H160, H256, U256};
use std::str::FromStr;
use evm_loader::account::EthereumAccount;
use log::debug;
use crate::{
    NeonCliResult, NeonCliError,
    program_options::truncate,
    Config,
    account_storage::account_info,
    commands::get_neon_elf::CachedElfParams,
};

#[derive(Clone)]
pub struct TxParams {
    pub from: H160,
    pub to: Option<H160>,
    pub data: Option<Vec<u8>>,
    pub value: Option<U256>,
    pub gas_limit: Option<U256>,
}

pub fn execute(cmd: &str, params: Option<&ArgMatches>, config: &Config) -> NeonCliResult{
    match (cmd, params) {
        ("emulate", Some(params)) => {
            let tx= parse_tx(params);
            let (token, chain, steps) = parse_tx_params(config, params);
            emulate::execute(config, &tx, token, chain, steps);
        }
        ("create-program-address", Some(params)) => {
            let ether = h160_of(params, "seed").expect("seed parse error");
            create_program_address::execute(config, &ether);
            Ok(())
        }
        ("create-ether-account", Some(params)) => {
            let ether = h160_of(params, "ether").expect("ether parse error");
            create_ether_account::execute(config, &ether)
        }
        ("deposit", Some(params)) => {
            let amount = value_of(params, "amount").expect("amount parse error");
            let ether = h160_of(params, "ether").expect("ether parse error");
            deposit::execute(config, amount, &ether)
        }
        ("get-ether-account-data", Some(params)) => {
            let ether = h160_of(params, "ether").expect("ether parse error");
            get_ether_account_data::execute(config, &ether);
            Ok(())
        }
        ("cancel-trx", Some(params)) => {
            let storage_account = pubkey_of(params, "storage_account").expect("storage_account parse error");
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
            let contract_id = h160_of(params, "contract_id").expect("contract_it parse error");
            let index = u256_of(params, "index").expect("index parse error");
            get_storage_at::execute(config, contract_id, &index);
            Ok(())
        }
        ("trace_call", Some(params)) => {
            let mut tx= parse_tx(params);
            tx.gas_limit = u256_of(params, "gas_limit");
            let (token, chain, steps) = parse_tx_params(config, params);
            trace_call::execute(config, &tx, token, chain, steps)
        }
        ("trace_trx", Some(params)) => {
            let tx = config.rpc_client.get_transaction_data().
                map_err(|_| NeonCliError::from(ClientErrorKind::Custom("load trx error".to_string())) )?;
            let (token, chain, steps) = parse_tx_params(config, params);
            trace_call::execute(config, &tx, token, chain, steps)
        }
            _ => unreachable!(),
    }
}

fn h160_or_deploy_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    if matches.value_of(name) == Some("deploy") {
        return None;
    }
    h160_of(matches, name)
}

fn h160_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    matches.value_of(name).map(|value| {
        // let err  = format!("{} parse error", name);
        H160::from_str(truncate(value)).unwrap_or_else(|_| panic!("{} parse error", name))
    })
}

fn u256_of(matches: &ArgMatches<'_>, name: &str) -> Option<U256> {
    matches.value_of(name).map(|value| {
        U256::from_str(truncate(value)).unwrap_or_else(|_| panic!("{} parse error", name))
    })
}

fn h256_of(matches: &ArgMatches<'_>, name: &str) -> Option<H256> {
    matches.value_of(name).map(|value| {
        H256::from_str(truncate(value)).unwrap_or_else(|_| panic!("{} parse error", name))
    })
}

fn read_stdin() -> Option<Vec<u8>>{
    let mut data = String::new();

    if let Ok(len) = std::io::stdin().read_line(&mut data){
        if len == 0{
            return None
        }
        let data = truncate(data.as_str());
        let bin = hex::decode(data).expect("data hex::decore error");
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



fn parse_tx(params: &ArgMatches) -> TxParams {
    let from = h160_of(params, "sender").expect("sender parse error");
    let to = h160_or_deploy_of(params, "contract");
    let data = read_stdin();
    let value = value_of(params, "value");

    TxParams {from, to, data, value, gas_limit: None}
}


pub fn parse_tx_params( config: &Config, params: &ArgMatches) -> (Pubkey, u64, u64) {
    // Read ELF params only if token_mint or chain_id is not set.
    let mut token = pubkey_of(params, "token_mint");
    let mut chain = value_of(params, "chain_id");
    if token.is_none() || chain.is_none() {
        let cached_elf_params = CachedElfParams::new(config);
        token = token.or_else(|| Some(Pubkey::from_str(
            cached_elf_params.get("NEON_TOKEN_MINT").expect("NEON_TOKEN_MINT load error")
        ).expect("NEON_TOKEN_MINT Pubkey ctor error ")));
        chain = chain.or_else(|| Some(u64::from_str(
            cached_elf_params.get("NEON_CHAIN_ID").expect("NEON_CHAIN_ID load error")
        ).expect("NEON_CHAIN_ID u64 ctor error")));
    }
    let token = token.expect("token_mint get error");
    let chain = chain.expect("chain_id get error");
    let max_steps = value_of::<u64>(params, "max_steps_to_execute").expect("max_steps_to_execute parse error");

    (token, chain, max_steps)
}