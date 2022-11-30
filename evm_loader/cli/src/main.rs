#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_possible_wrap)]

mod account_storage;
mod syscall_stubs;

mod errors;
mod logs;
mod commands;
mod rpc;
mod program_options;

use crate::{
    account_storage::{
        make_solana_program_address,
        account_info,
    },
    commands::{
        emulate,
        create_program_address,
        create_ether_account,
        deposit,
        get_ether_account_data,
        cancel_trx,
        get_neon_elf,
        collect_treasury,
        init_environment,
        get_storage_at,
    },
    rpc::{Rpc, db::PostgresClient},
};

use evm_loader::{
    account::{
        EthereumAccount,
    },
};

use evm::{H160, H256, U256};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, Signature},
    transaction::Transaction,
};
use std::{
    io::{Read},
    fs::File,
    str::FromStr,
    process::{exit},
    sync::Arc,
    fmt,
    fmt::Debug,
};

use clap::ArgMatches;

use solana_clap_utils::{
    input_parsers::{pubkey_of, value_of,},
    input_validators::normalize_to_url_if_moniker,
    keypair::{signer_from_path, keypair_from_path},
};

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig},
    client_error::Result as SolanaClientResult,
};

use rlp::RlpStream;

use log::{ debug, error};
use logs::LogContext;

use crate::{
    errors::NeonCliError,
    program_options::{parse_options, make_clean_hex},
    get_neon_elf::CachedElfParams, rpc::{DB_INSTANCE, NODE_INSTANCE},
};

type NeonCliResult = Result<(),NeonCliError>;

pub struct Config {
    rpc_client: rpc::Clients,
    evm_loader: Pubkey,
    signer: Box<dyn Signer>,
    fee_payer: Option<Keypair>,
    commitment: CommitmentConfig,
}

impl Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "evm_loader={:?}, signer={:?}", self.evm_loader, self.signer)
    }
}

/// Read program data from file
/// # Errors
/// Returns `Err` if can't read from file
pub fn read_program_data(program_location: &str) -> Result<Vec<u8>, NeonCliError> {
    let mut file = File::open(program_location)?;
    // let mut file = File::open(program_location).map_err(|err| {
    //     format!("Unable to open program file '{}': {}", program_location, err)
    // })?;
    let mut program_data = Vec::new();
    file.read_to_end(&mut program_data)?;
    // file.read_to_end(&mut program_data).map_err(|err| {
    //     format!("Unable to read program file '{}': {}", program_location, err)
    // })?;

    Ok(program_data)
}

#[must_use]
pub fn keccak256_h256(data: &[u8]) -> H256 {
    let hash = solana_sdk::keccak::hash(data).to_bytes();
    H256::from(hash)
}

#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    solana_sdk::keccak::hash(data).to_bytes()
}

#[must_use]
pub fn keccak256_digest(data: &[u8]) -> Vec<u8> {
    solana_sdk::keccak::hash(data).to_bytes().to_vec()
}

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

// fn get_ethereum_caller_credentials(
//     config: &Config,
// ) -> (SecretKey, H160, Pubkey, u8, Pubkey, Pubkey) {
//     use secp256k1::PublicKey;
//     let caller_private = {
//         let private_bytes : [u8; 64] = config.keypair.as_ref().unwrap().to_bytes();
//         let mut sign_arr: [u8;32] = Default::default();
//         sign_arr.clone_from_slice(&private_bytes[..32]);
//         SecretKey::parse(&sign_arr).unwrap()
//     };
//     let caller_public = PublicKey::from_secret_key(&caller_private);
//     let caller_ether: H160 = keccak256_h256(&caller_public.serialize()[1..]).into();
//     let (caller_sol, caller_nonce) = make_solana_program_address(&caller_ether, &config.evm_loader);
//     let caller_token = spl_associated_token_account::get_associated_token_address(&caller_sol, &evm_loader::neon::token_mint::id());
//     let caller_holder = create_block_token_account(config, &caller_ether, &caller_sol).unwrap();
//     debug!("caller_sol = {}", caller_sol);
//     debug!("caller_ether = {}", caller_ether);
//     debug!("caller_token = {}", caller_token);

//     (caller_private, caller_ether, caller_sol, caller_nonce, caller_token, caller_holder)
// }

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

fn get_program_ether(
    caller_ether: &H160,
    trx_count: u64
) -> H160 {
    let trx_count_256 : U256 = U256::from(trx_count);
    let mut stream = rlp::RlpStream::new_list(2);
    stream.append(caller_ether);
    stream.append(&trx_count_256);
    keccak256_h256(&stream.out()).into()
}


fn send_transaction(
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

    // Ok(tx_sig)
}



// Return H160 for an argument
fn h160_or_deploy_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    if matches.value_of(name) == Some("deploy") {
        return None;
    }
    matches.value_of(name).map(|value| {
        H160::from_str(make_clean_hex(value)).unwrap()
    })
}


// Return H160 for an argument
fn h160_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    matches.value_of(name).map(|value| {
        H160::from_str(make_clean_hex(value)).unwrap()
    })
}

// Return U256 for an argument
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



#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    let app_matches = parse_options();
    let context: LogContext =
        app_matches.value_of("logging_ctx")
            .map(|ctx| LogContext::new(ctx.to_string()) )
            .unwrap_or_default();
    let loglevel: log::LevelFilter =
        app_matches.value_of("loglevel")
            .map_or(log::LevelFilter::Trace, |ll| 
                match ll.to_ascii_lowercase().as_str() {
                    "off"   => log::LevelFilter::Off,
                    "error" => log::LevelFilter::Error,
                    "warn"  => log::LevelFilter::Warn,
                    "info"  => log::LevelFilter::Info,
                    "debug" => log::LevelFilter::Debug,
                    _       => log::LevelFilter::Trace,
                }
            );
    logs::init(context, loglevel).unwrap();

    let mut wallet_manager = None;
    let config = {
        let cli_config = app_matches.value_of("config_file").map_or_else(
            solana_cli_config::Config::default,
            |config_file| solana_cli_config::Config::load(config_file).unwrap_or_default()
        );

        let commitment = CommitmentConfig::from_str(app_matches.value_of("commitment").unwrap()).unwrap();

        let json_rpc_url = normalize_to_url_if_moniker(
            app_matches
                .value_of("json_rpc_url")
                .unwrap_or(&cli_config.json_rpc_url),
        );

        let evm_loader = 
            if let Some(evm_loader) = pubkey_of(&app_matches, "evm_loader") {
                evm_loader
            } else {
                let e = NeonCliError::EvmLoaderNotSpecified;
                error!("{}", e);
                exit(e.error_code() as i32);
            };

        let signer = signer_from_path(
            &app_matches,
            app_matches
                .value_of("keypair")
                .unwrap_or(&cli_config.keypair_path),
            "keypair",
            &mut wallet_manager,
        ).unwrap_or_else(
            |_| {
                let e = NeonCliError::KeypairNotSpecified;
                error!("{}", e);
                exit(e.error_code() as i32);
            }
        );

        let fee_payer = keypair_from_path(
            &app_matches,
            app_matches
                .value_of("fee_payer")
                .unwrap_or(&cli_config.keypair_path),
            "fee_payer",
            true,
        ).ok();


        let rpc_client = if let Some(slot) = app_matches.value_of("slot") {
            let slot:u64 = slot.parse().unwrap();

            let db_config = app_matches.value_of("db_config")
                .map(|path|{ solana_cli_config::load_config_file(path).unwrap()})
                .unwrap();

            DB_INSTANCE.set(Arc::new(PostgresClient::new(&db_config, slot))).unwrap();
            rpc::Clients::Postgress
        } else {
            NODE_INSTANCE.set(
                Arc::new(RpcClient::new_with_commitment(json_rpc_url, commitment))
            ).unwrap_or_else(|_|{
                error!("NODE_INSTANCE.set error");
                exit(NeonCliError::UnknownError.error_code() as i32);
            });
            rpc::Clients::Node
        };

        Config {
            rpc_client,
            evm_loader,
            signer,
            fee_payer,
            commitment,
        }
    };

    let (sub_command, sub_matches) = app_matches.subcommand();
    let result: NeonCliResult =
        match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_or_deploy_of(arg_matches, "contract");
                let sender = h160_of(arg_matches, "sender").unwrap();
                let data = read_stdin();
                let value = value_of(arg_matches, "value");

                // Read ELF params only if token_mint or chain_id is not set.
                let mut token_mint = pubkey_of(arg_matches, "token_mint");
                let mut chain_id = value_of(arg_matches, "chain_id");
                if token_mint.is_none() || chain_id.is_none() {
                    let cached_elf_params = CachedElfParams::new(&config);
                    token_mint = token_mint.or_else(|| Some(Pubkey::from_str(
                        cached_elf_params.get("NEON_TOKEN_MINT").unwrap()
                    ).unwrap()));
                    chain_id = chain_id.or_else(|| Some(u64::from_str(
                        cached_elf_params.get("NEON_CHAIN_ID").unwrap()
                    ).unwrap()));
                }
                let token_mint = token_mint.unwrap();
                let chain_id = chain_id.unwrap();
                let max_steps_to_execute = value_of::<u64>(arg_matches, "max_steps_to_execute").unwrap();

                emulate::execute(&config,
                                 contract,
                                 sender,
                                 data,
                                 value,
                                 &token_mint,
                                 chain_id,
                                 max_steps_to_execute)
            }
            ("create-program-address", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "seed").unwrap();
                create_program_address::execute(&config, &ether);
                Ok(())
            }
            ("create-ether-account", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "ether").unwrap();
                create_ether_account::execute(&config, &ether)
            }
            ("deposit", Some(arg_matches)) => {
                let amount = value_of(arg_matches, "amount").unwrap();
                let ether = h160_of(arg_matches, "ether").unwrap();
                deposit::execute(&config, amount, &ether)
            }
            ("get-ether-account-data", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "ether").unwrap();
                get_ether_account_data::execute(&config, &ether);
                Ok(())
            }
            ("cancel-trx", Some(arg_matches)) => {
                let storage_account = pubkey_of(arg_matches, "storage_account").unwrap();
                cancel_trx::execute(&config, &storage_account)
            }
            ("neon-elf-params", Some(arg_matches)) => {
                let program_location = arg_matches.value_of("program_location");
                get_neon_elf::execute(&config, program_location)
            }
            ("collect-treasury", Some(_)) => {
                collect_treasury::execute(&config)
            }
            ("init-environment", Some(arg_matches)) => {
                let file = arg_matches.value_of("file");
                let send_trx = arg_matches.is_present("send-trx");
                let force = arg_matches.is_present("force");
                let keys_dir = arg_matches.value_of("keys-dir");
                init_environment::execute(&config, send_trx, force, keys_dir, file)
            }
            ("get-storage-at", Some(arg_matches)) => {
                let contract_id = h160_of(arg_matches, "contract_id").unwrap();
                let index = u256_of(arg_matches, "index").unwrap();
                get_storage_at::execute(&config, contract_id, &index);
                Ok(())
            }
            _ => unreachable!(),
        };
    
    let exit_code: i32 =
        match result {
            Ok(_)  => 0,
            Err(e) => {
                let error_code = e.error_code();
                error!("NeonCli Error ({}): {}", error_code, e);
                error_code as i32
            }
        };
    
    exit(exit_code);
}
