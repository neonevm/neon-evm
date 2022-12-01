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
use evm::{H160, H256, U256};
use std::str::FromStr;
use evm_loader::account::EthereumAccount;
use log::debug;
use crate::{
    NeonCliResult, NeonCliError,
    program_options::make_clean_hex,
    commands::get_neon_elf::CachedElfParams,
    Config,
    rpc::Rpc,
    account_storage::account_info,
};
use rlp::RlpStream;

#[derive(Debug)]
pub struct UnsignedTransaction {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<H160>,
    pub value: U256,
    pub data: Vec<u8>,
    pub chain_id: U256,
}

impl rlp::Encodable for UnsignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(addr) => s.append(addr),
        };
        s.append(&self.value);
        s.append(&self.data);
        s.append(&self.chain_id);
        s.append_empty_data();
        s.append_empty_data();
    }
}

pub fn execute(cmd: &str, params: Option<&ArgMatches>, config: &Config) -> NeonCliResult{
    match (cmd, params) {
        ("emulate", Some(params)) => {
            let contract = h160_or_deploy_of(params, "contract");
            let sender = h160_of(params, "sender").unwrap();
            let data = read_stdin();
            let value = value_of(params, "value");

            // Read ELF params only if token_mint or chain_id is not set.
            let mut token_mint = pubkey_of(params, "token_mint");
            let mut chain_id = value_of(params, "chain_id");
            if token_mint.is_none() || chain_id.is_none() {
                let cached_elf_params = CachedElfParams::new(config);
                token_mint = token_mint.or_else(|| Some(Pubkey::from_str(
                    cached_elf_params.get("NEON_TOKEN_MINT").unwrap()
                ).unwrap()));
                chain_id = chain_id.or_else(|| Some(u64::from_str(
                    cached_elf_params.get("NEON_CHAIN_ID").unwrap()
                ).unwrap()));
            }
            let token_mint = token_mint.unwrap();
            let chain_id = chain_id.unwrap();
            let max_steps_to_execute = value_of::<u64>(params, "max_steps_to_execute").unwrap();

            emulate::execute(config,
                             contract,
                             sender,
                             data,
                             value,
                             &token_mint,
                             chain_id,
                             max_steps_to_execute)
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

