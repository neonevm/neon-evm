#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_possible_wrap)]

mod account_storage;
mod syscall_stubs;

mod errors;
mod logs;
mod commands;

use crate::{
    account_storage::{
        make_solana_program_address,
        account_info,
    },
    commands::{
        emulate,
        create_program_address,
        create_ether_account,
        deploy,
        deposit,
        migrate_account,
        get_ether_account_data,
        cancel_trx,
        get_neon_elf,
        get_storage_at,
        update_valids_table,
    },
};

use evm_loader::{
    account::{
        ACCOUNT_SEED_VERSION,
        EthereumAccount,
    },
    config::{  collateral_pool_base },
};

use evm::{H160, H256, U256};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, Signature},
    keccak::Hasher,
    transaction::Transaction,
    system_instruction,
};
use std::{
    io::{Read},
    fs::File,
    env,
    str::FromStr,
    process::{exit},
    sync::Arc,
    convert::{TryInto},
    fmt,
    fmt::{Debug, Display,},
};

use clap::{
    crate_description, crate_name, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_program::{
    keccak::{hash,},
};

use solana_clap_utils::{
    input_parsers::{pubkey_of, value_of,},
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker},
    keypair::{signer_from_path, keypair_from_path},
};

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig},
    client_error::Result as SolanaClientResult,
};

use rlp::RlpStream;

use log::{debug, error};
use logs::LogContext;

use crate::errors::NeonCliError;
use crate::get_neon_elf::CachedElfParams;

type NeonCliResult = Result<(),NeonCliError>;

pub struct Config {
    rpc_client: Arc<RpcClient>,
    websocket_url: String,
    evm_loader: Pubkey,
    // #[allow(unused)]
    // fee_payer: Pubkey,
    signer: Box<dyn Signer>,
    keypair: Option<Keypair>,
    commitment: CommitmentConfig,
}

impl Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "evm_loader={:?}, signer={:?}", self.evm_loader, self.signer)
    }
}

fn read_program_data(program_location: &str) -> Result<Vec<u8>, NeonCliError> {
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
    H256::from(hash(data).to_bytes())
}

#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    hash(data).to_bytes()
}

#[must_use]
pub fn keccak256_digest(data: &[u8]) -> Vec<u8> {
    hash(data).to_bytes().to_vec()
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

fn create_storage_account(config: &Config) -> SolanaClientResult<Pubkey> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let creator = &config.signer;
    debug!("Create storage account");
    let storage = create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &rng.gen::<u32>().to_string(), 128*1024_u64)?;
    debug!("storage = {}", storage);
    Ok(storage)
}

fn get_collateral_pool_account_and_index(config: &Config, collateral_pool_base: &Pubkey) -> (Pubkey, u32) {
    let collateral_pool_index = 2;
    let seed = format!("{}{}", collateral_pool_base::PREFIX, collateral_pool_index);
    let collateral_pool_account = Pubkey::create_with_seed(
        collateral_pool_base,
        &seed,
        &config.evm_loader).unwrap();

    (collateral_pool_account, collateral_pool_index)
}

fn create_account_with_seed(
    config: &Config,
    funding: &Pubkey,
    base: &Pubkey,
    seed: &str,
    len: u64
) -> SolanaClientResult<Pubkey> {
    let created_account = Pubkey::create_with_seed(base, seed, &config.evm_loader).unwrap();

    if config.rpc_client.get_account_with_commitment(&created_account, CommitmentConfig::confirmed())?.value.is_none() {
        debug!("Account not found");
        let minimum_balance_for_account = config.rpc_client.get_minimum_balance_for_rent_exemption(len.try_into().unwrap())?;
        let create_acc_instruction = system_instruction::create_account_with_seed(
            funding,
            &created_account,
            base,
            seed,
            minimum_balance_for_account,
            len,
            &config.evm_loader
        );
        send_transaction(config, &[create_acc_instruction])?;
    } else {
        debug!("Account found");
    }

    Ok(created_account)
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

/// Returns random nonce and the corresponding seed.
fn generate_random_holder_seed() -> (u64, String) {
    use rand::Rng as _;
    // proxy_id_bytes = proxy_id.to_bytes((proxy_id.bit_length() + 7) // 8, 'big')
    // seed = keccak_256(b'holder' + proxy_id_bytes).hexdigest()[:32]
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen();
    let bytes_count = std::mem::size_of_val(&id);
    let bits_count = bytes_count * 8;
    let holder_id_bit_length = bits_count - id.leading_zeros() as usize;
    let significant_bytes_count = (holder_id_bit_length + 7) / 8;
    let mut hasher = Hasher::default();
    hasher.hash(b"holder");
    hasher.hash(&id.to_be_bytes()[bytes_count-significant_bytes_count..]);
    let output = hasher.result();
    (id, hex::encode(output)[..32].into())
}

fn make_clean_hex(in_str: &str) -> &str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {
        in_str
    }
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

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160_or_deploy<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    if string.as_ref() == "deploy" {
        return Ok(());
    }
    H160::from_str(make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
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

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    H160::from_str(make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return an error if string cannot be parsed as a U256 integer
fn is_valid_u256<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    U256::from_str(make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return hexdata for an argument
fn hexdata_of(matches: &ArgMatches<'_>, name: &str) -> Option<Vec<u8>> {
    matches.value_of(name).and_then(|value| {
        if value.to_lowercase() == "none" {
            return None;
        }
        hex::decode(&make_clean_hex(value)).ok()
    })
}

// Return an error if string cannot be parsed as a hexdata
fn is_valid_hexdata<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    if string.as_ref().to_lowercase() == "none" {
        return Ok(());
    }

    hex::decode(&make_clean_hex(string.as_ref())).map(|_| ())
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
            "Unable to parse input amount as {}, provided: {}",
            std::any::type_name::<T>(), amount
        ))
    }
}

macro_rules! neon_cli_pkg_version {
    () => ( env!("CARGO_PKG_VERSION") )
}
macro_rules! neon_cli_revision {
    () => ( env!("NEON_REVISION") )
}
macro_rules! version_string {
    () => ( concat!("Neon-cli/v", neon_cli_pkg_version!(), "-", neon_cli_revision!()) )
}


#[allow(clippy::too_many_lines)]
fn main() {
    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(version_string!())
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
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .global(true)
                .multiple(true)
                .help("Increase message verbosity"),
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
                .value_name("LOG_CONTEST")
                .takes_value(true)
                .global(true)
                .help("Logging context"),
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
                        .validator(is_valid_h160_or_deploy)
                        .help("The contract that executes the transaction or 'deploy'")
                )
                .arg(
                    Arg::with_name("data")
                        .value_name("DATA")
                        .takes_value(true)
                        .index(3)
                        .required(false)
                        .validator(is_valid_hexdata)
                        .help("Transaction data or 'None'")
                )
                .arg(
                    Arg::with_name("value")
                        .value_name("VALUE")
                        .takes_value(true)
                        .index(4)
                        .required(false)
                        .validator(is_amount::<U256, _>)
                        .help("Transaction value")
                )
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
                .arg(
                    Arg::with_name("collateral_pool_base")
                        .long("collateral_pool_base")
                        .value_name("COLLATERAL_POOL_BASE")
                        .takes_value(true)
                        .global(true)
                        .validator(is_valid_pubkey)
                        .help("Collateral_pool_base public key")
                )
                .arg(
                    Arg::with_name("chain_id")
                        .long("chain_id")
                        .value_name("CHAIN_ID")
                        .takes_value(true)
                        .required(false)
                        .help("Network chain_id"),
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
            SubCommand::with_name("migrate-account")
                .about("Migrates account internal structure to v2")
                .arg(
                    Arg::with_name("ether")
                        .index(1)
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
        .subcommand(
            SubCommand::with_name("update-valids-table")
                .about("Update Valids Table")
                .arg(
                    Arg::with_name("contract_id")
                        .index(1)
                        .value_name("contract_id")
                        .takes_value(true)
                        .validator(is_valid_h160)
                        .required(true),
                )
        )
        .get_matches();

    let context: LogContext =
        app_matches.value_of("logging_ctx")
            .map(|ctx| LogContext::new(ctx.to_string()) )
            .unwrap_or_default();
    logs::init(context).unwrap();

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

        let (signer, _fee_payer) = signer_from_path(
            &app_matches,
            app_matches
                .value_of("fee_payer")
                .unwrap_or(&cli_config.keypair_path),
            "fee_payer",
            &mut wallet_manager,
        ).map_or_else(
            |e| {
                error!("{}", e);
                let e = NeonCliError::FeePayerNotSpecified;
                error!("{}", e);
                exit(e.error_code() as i32);
            },
            |s| {
                let p = s.pubkey();
                (s, p)
            }
        );

        let keypair = keypair_from_path(
            &app_matches,
            app_matches
                .value_of("fee_payer")
                .unwrap_or(&cli_config.keypair_path),
            "fee_payer",
            true,
        ).ok();

        Config {
            rpc_client: Arc::new(RpcClient::new_with_commitment(json_rpc_url, commitment)),
            websocket_url: "".to_string(),
            evm_loader,
            signer,
            keypair,
            commitment,
        }
    };

    let (sub_command, sub_matches) = app_matches.subcommand();
    let result: NeonCliResult =
        match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_or_deploy_of(arg_matches, "contract");
                let sender = h160_of(arg_matches, "sender").unwrap();
                let data = hexdata_of(arg_matches, "data");
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

                emulate::execute(&config, contract, sender, data, value, &token_mint, chain_id)
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
            ("deploy", Some(arg_matches)) => {
                let program_location = arg_matches.value_of("program_location").unwrap().to_string();

                // Read ELF params only if collateral_pool_base or chain_id is not set.
                let mut collateral_pool_base = pubkey_of(arg_matches, "collateral_pool_base");
                let mut chain_id = value_of(arg_matches, "chain_id");
                if collateral_pool_base.is_none() || chain_id.is_none() {
                    let cached_elf_params = CachedElfParams::new(&config);
                    collateral_pool_base = collateral_pool_base.or_else(|| Some(Pubkey::from_str(
                        cached_elf_params.get("NEON_POOL_BASE").unwrap()
                    ).unwrap()));
                    chain_id = chain_id.or_else(|| Some(u64::from_str(
                        cached_elf_params.get("NEON_CHAIN_ID").unwrap()
                    ).unwrap()));
                }
                let collateral_pool_base = collateral_pool_base.unwrap();
                let chain_id = chain_id.unwrap();

                deploy::execute(&config, &program_location, &collateral_pool_base, chain_id)
            }
            ("deposit", Some(arg_matches)) => {
                let amount = value_of(arg_matches, "amount").unwrap();
                let ether = h160_of(arg_matches, "ether").unwrap();
                deposit::execute(&config, amount, &ether)
            }
            ("migrate-account", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "ether").unwrap();
                migrate_account::execute(&config, &ether)
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
            ("get-storage-at", Some(arg_matches)) => {
                let contract_id = h160_of(arg_matches, "contract_id").unwrap();
                let index = u256_of(arg_matches, "index").unwrap();
                get_storage_at::execute(&config, contract_id, &index);
                Ok(())
            }
            ("update-valids-table", Some(arg_matches)) => {
                let contract_id = h160_of(arg_matches, "contract_id").unwrap();
                update_valids_table::execute(&config, contract_id)
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
    
    exit(exit_code)
}
