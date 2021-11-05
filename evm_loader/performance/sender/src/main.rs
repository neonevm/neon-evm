use std::fs::File;
use std::vec::Vec;
use std::time::{Duration, SystemTime};
use std::{thread, time};

use std::{
    rc::Rc,
    sync::Arc,
    i64,
    str::FromStr,
    process::exit,
    fmt::Display,
    env
};

use std::io::{self, prelude::*, BufReader};

use serde::{Deserialize, Serialize};

use solana_sdk::{
    clock::Slot,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{AccountMeta, Instruction},
    loader_instruction::LoaderInstruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, Signature},
    signers::Signers,
    transaction::Transaction,
    system_program,
    system_instruction,
    sysvar::{clock},
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


use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcTransactionConfig},
    rpc_request::MAX_GET_SIGNATURE_STATUSES_QUERY_ITEMS,
    tpu_client::{TpuClient, TpuClientConfig},
};
use solana_cli::{
    checks::{check_account_for_fee},
};
use solana_cli_output::display::new_spinner_progress_bar;
use solana_transaction_status::{
    TransactionConfirmationStatus,
    UiTransactionEncoding,
    EncodedTransaction,
    UiMessage,
    UiInstruction,
    EncodedConfirmedTransaction
};
use std::borrow::{Borrow, BorrowMut};

use evm_loader::{
    instruction::EvmInstruction,
    // solana_backend::SolanaBackend,
    account_data::{AccountData, Account, Contract},
};
use evm::{H160, H256, U256};
use solana_sdk::recent_blockhashes_account::update_account;

#[derive(Serialize, Deserialize)]
struct trx_t {
    from_addr: String,
    sign: String,
    msg: String,
    erc20_sol: String,
    erc20_eth: String,
    erc20_code: String,
    payer_sol: String,
    payer_eth: String,
    receiver_eth: String,
}

#[derive(Serialize, Deserialize)]
struct sender_t{
    pub_key: String,
    pr_key: String,
}

#[derive(Serialize, Deserialize)]
struct collateral_t{
    account : String,
    index: u32
}

fn make_keccak_instruction_data(instruction_index : u8, msg_len: u16, data_start : u16) ->Vec<u8> {
    let mut data = Vec::new();

    let check_count : u8 = 1;
    let eth_address_size : u16 = 20;
    let signature_size : u16 = 65;
    let eth_address_offset: u16 = data_start;
    let signature_offset : u16 = eth_address_offset + eth_address_size;
    let message_data_offset : u16 = signature_offset + signature_size;

    data.push(check_count);

    data.push(signature_offset as u8);
    data.push((signature_offset >> 8) as u8);

    data.push(instruction_index);

    data.push(eth_address_offset as u8);
    data.push((eth_address_offset >> 8) as u8);

    data.push(instruction_index);

    data.push(message_data_offset as u8);
    data.push((message_data_offset >> 8) as u8);

    data.push(msg_len as u8);
    data.push((msg_len >> 8) as u8);

    data.push(instruction_index);
    return data;
}

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

fn parse_program_args() -> (Pubkey, String, String, String, String, String, String, u64) {
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
        Arg::with_name("transaction_file")
            .value_name("TRANSACTION_FILEPATH")
            .takes_value(true)
            .required(true)
            .help("/path/to/transaction.json")
            .default_value("transaction.json"),
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

    let trx_filename = app_matches.value_of("transaction_file").unwrap().to_string();
    let senders_filename = app_matches.value_of("sender_file").unwrap().to_string();
    let verify_filename = app_matches.value_of("verify_file").unwrap().to_string();
    let collateral_filename = app_matches.value_of("collateral_file").unwrap().to_string();
    let delay :u64 = app_matches.value_of("delay").unwrap().to_string().parse().unwrap();

    return (evm_loader, json_rpc_url, trx_filename, senders_filename, verify_filename, collateral_filename, client, delay);
}

fn read_senders(filename: &String) -> Result<Vec<Vec<u8>>, Error>{
    let mut file = File::open(filename)?;
    let reader= BufReader::new(file);
    let mut keys = Vec::new();

    for line in reader.lines(){
        let bin = hex::decode(line?.as_str()).unwrap();
        keys.push(bin);
    }
    return Ok(keys);
}

fn read_collateral(filename: &String) -> Result<Vec<collateral_t>, Error> {
    let mut file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut pool = Vec::new();

    for line in reader.lines() {
        let data: collateral_t = serde_json::from_str(line?.as_str())?;
        pool.push(data);
    }
    return Ok(pool);
}

fn make_instruction_budget_units() -> Instruction{
    let DEFAULT_UNITS:u32 =500*1000;

    let instruction_unit = Instruction::new_with_bincode(
        Pubkey::from_str("ComputeBudget111111111111111111111111111111").unwrap(),
        &(0x00_u8, DEFAULT_UNITS),
        vec![]);

    instruction_unit
}

fn make_instruction_budget_heap() -> Instruction{
    let DEFAULT_HEAP_FRAME: u32=256*1024;

    let instruction_heap = Instruction::new_with_bincode(
        Pubkey::from_str("ComputeBudget111111111111111111111111111111").unwrap(),
        &(0x01_u8, DEFAULT_HEAP_FRAME),
        vec![]);

    instruction_heap
}

fn make_instruction_05(trx : &trx_t, evm_loader_key : &Pubkey, operator_sol : &Pubkey, collateral: &collateral_t) -> Instruction {

    let mut data_05_hex = String::from("05");
    data_05_hex.push_str(hex::encode(collateral.index.to_le_bytes()).as_str());
    data_05_hex.push_str(trx.from_addr.as_str());
    data_05_hex.push_str(trx.sign.as_str());
    data_05_hex.push_str(trx.msg.as_str());
    let data_05 : Vec<u8> = hex::decode(data_05_hex.as_str()).unwrap();

    let contract = Pubkey::from_str(trx.erc20_sol.as_str()).unwrap();
    let caller = Pubkey::from_str(trx.payer_sol.as_str()).unwrap();
    let sysinstruct = Pubkey::from_str("Sysvar1nstructions1111111111111111111111111").unwrap();
    let sysvarclock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();
    let system = Pubkey::from_str("11111111111111111111111111111111").unwrap();
    let token_id = Pubkey::from_str("89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g").unwrap();
    let contract_token = spl_associated_token_account::get_associated_token_address(&contract, &token_id);
    let caller_token = spl_associated_token_account::get_associated_token_address(&caller, &token_id);
    let operator_token = spl_associated_token_account::get_associated_token_address(&operator_sol, &token_id);
    let collateral_pool_acc = Pubkey::from_str(collateral.account.as_str()).unwrap();

    let mut acc_meta = vec![

        AccountMeta::new_readonly(sysinstruct, false),
        AccountMeta::new(*operator_sol, true),
        AccountMeta::new(collateral_pool_acc, false),
        AccountMeta::new(operator_token, false),
        AccountMeta::new(caller_token, false),
        AccountMeta::new(system, false),

        AccountMeta::new(contract, false),
        AccountMeta::new(contract_token, false),
        // AccountMeta::new(contract_code, false),
        AccountMeta::new(caller, false),
        AccountMeta::new(caller_token, false),

        AccountMeta::new_readonly(*evm_loader_key, false),
        AccountMeta::new_readonly(token_id, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(sysvarclock, false),
    ];

    if (trx.erc20_code != ""){
        let contract_code = Pubkey::from_str(trx.erc20_code.as_str()).unwrap();
        acc_meta.insert(8, AccountMeta::new(contract_code, false));
    }

    let instruction_05 = Instruction::new_with_bytes(
        *evm_loader_key,
        &data_05,
        acc_meta);

    instruction_05
}

fn create_trx(
    evm_loader: &Pubkey,
    trx_filename: &String,
    senders_filename :&String,
    collateral_filename: &String,
    rpc_client: &Arc<RpcClient> )-> Result<Vec<(Transaction, String, String, String)>, Error>{

    let keccakprog = Pubkey::from_str("KeccakSecp256k11111111111111111111111111111").unwrap();

    let mut keys = read_senders(&senders_filename).unwrap();
    let mut collaterals = read_collateral(&collateral_filename).unwrap();

    println!("creating transactions  ..");
    let mut transaction = Vec::new();
    let mut it_keys = keys.iter();
    let mut it_collaterals = collaterals.iter();

    let mut trx_file = File::open(trx_filename)?;
    let trx_reader= BufReader::new(trx_file);

    let mut collateral_file = File::open(collateral_filename)?;
    let collateral_reader= BufReader::new(collateral_file);

    for line in trx_reader.lines(){

        let mut keypair_bin : &Vec<u8>;
        match (it_keys.next()){
            Some(val) => keypair_bin = val,
            None => {
                it_keys = keys.iter();
                keypair_bin = it_keys.next().unwrap()
            }
        }

        let mut collateral_data : &collateral_t;
        match (it_collaterals.next()){
            Some(val) => collateral_data = val,
            None => {
                it_collaterals = collaterals.iter();
                collateral_data = it_collaterals.next().unwrap()
            }
        }

        let keypair =Keypair::from_bytes(keypair_bin).unwrap();
        let trx : trx_t = serde_json::from_str(line?.as_str())?;
        let msg = hex::decode(&trx.msg).unwrap();

        let data_keccak = make_keccak_instruction_data(1, msg.len() as u16, 5);
        let instruction_keccak = Instruction::new_with_bytes(
            keccakprog,
            &data_keccak,
            vec![
                AccountMeta::new_readonly(keccakprog, false),
            ]
        );
        let keypair_pubkey = keypair.pubkey();
        let signer: Box<dyn Signer> = Box::from(keypair);
        let instruction_05 = make_instruction_05(&trx, evm_loader, &signer.pubkey(), collateral_data);
        // let instruction_budget_units = make_instruction_budget_units();
        // let instruction_budget_heap = make_instruction_budget_heap();


        let message = Message::new(
            // &[instruction_budget_units, instruction_budget_heap, instruction_keccak, instruction_05],
            &[instruction_keccak, instruction_05],
            Some(&keypair_pubkey)
        );
        let mut tx = Transaction::new_unsigned(message);

        let blockhash : solana_program::hash::Hash;
        match (rpc_client.get_recent_blockhash()){
            Ok((hash,_)) => blockhash = hash,
            _ => panic!("get_recent_blockhash() error")
        }

        tx.try_sign(&[&*signer] , blockhash)?;
        transaction.push((tx, trx.erc20_eth, trx.payer_eth, trx.receiver_eth));
    }

    return Ok(transaction);
}

fn write_for_verify(verify_filename : &String, signatures: &Vec<(String, String, String, Signature)>)
    -> Result<(), Error>{
    let mut verify = File::create(verify_filename).unwrap();

    // Write a &str in the file (ignoring the result).
    for (erc20_eth, payer_eth, receiver_eth, sig) in signatures{
        writeln!(&mut verify, "[\"{}\", \"{}\", \"{}\", \"{}\"]",
                 &erc20_eth.to_string(),
                 &payer_eth.to_string(),
                 &receiver_eth.to_string(),
                 &sig.to_string()
        ).unwrap();

    }
    return Ok(());
}

fn main() -> CommandResult{

    let (evm_loader,
        json_rpc_url,
        trx_filename,
        senders_filename,
        verify_filename,
        collateral_filename,
        client,
        delay
    )
        = parse_program_args();

    let rpc_client = Arc::new(RpcClient::new_with_commitment(json_rpc_url,
                                                            CommitmentConfig::confirmed()));

    let transaction = create_trx(&evm_loader, &trx_filename, &senders_filename, &collateral_filename, &rpc_client).unwrap();

    println!("sending transactions ..");
    let mut count = 0;
    let mut signatures = Vec::new();
    let tpu_config : TpuClientConfig = TpuClientConfig::default();
    let tpu_client = TpuClient::new(rpc_client.clone(), "", tpu_config).unwrap();
    let ten = time::Duration::from_micros(delay);
    let start = SystemTime::now();
    for (tx, erc20_eth, payer_eth, receiver_eth) in transaction{
        if (client == "tcp"){
            let sig = rpc_client.send_transaction_with_config(
                &tx,
                RpcSendTransactionConfig {
                    skip_preflight : true,
                    preflight_commitment: Some(CommitmentLevel::Confirmed),
                    ..RpcSendTransactionConfig::default()
                }
            )?;
            signatures.push((erc20_eth, payer_eth, receiver_eth, sig));
        }
        else if (client == "udp") {
            let res = tpu_client.send_transaction(&tx);
            signatures.push((erc20_eth, payer_eth, receiver_eth, tx.signatures[0]));
        }
        count = count  + 1;
        thread::sleep(ten);
    }
    let end = SystemTime::now();
    let time = end.duration_since(start).expect("Clock may have gone backwards");
    println!("time  {:?}", time);
    println!("count {}", &count.to_string());

    write_for_verify(&verify_filename, &signatures);

    Ok(())
}

