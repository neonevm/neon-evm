use std::fs::File;
use std::vec::Vec;
use std::time::{Duration, SystemTime};

use std::{
    rc::Rc,
    i64,
    str::FromStr,
    process::exit,
    fmt::Display,
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
    rpc_config::{RpcSendTransactionConfig, RpcConfirmedTransactionConfig},
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
    solana_backend::SolanaBackend,
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

#[derive(Default, Serialize, Deserialize, Debug)]
struct SecpSignatureOffsets {
    signature_offset: u16, // offset to [signature,recovery_id] of 64+1 bytes
    signature_instruction_index: u8,
    eth_address_offset: u16, // offset to eth_address of 20 bytes
    eth_address_instruction_index: u8,
    message_data_offset: u16, // offset to start of message data
    message_data_size: u16,   // size of message data
    message_instruction_index: u8,
}

pub fn make_secp256k1_instruction(instruction_index: u8, message_len: u16, data_start: u16) -> Vec<u8> {
    const NUMBER_OF_SIGNATURES: u8 = 1;
    const ETH_SIZE: u16 = 20;
    const SIGN_SIZE: u16 = 65;
    let eth_offset: u16 = data_start;
    let sign_offset: u16 = eth_offset + ETH_SIZE;
    let msg_offset: u16 = sign_offset + SIGN_SIZE;

    let offsets = SecpSignatureOffsets {
        signature_offset: sign_offset,
        signature_instruction_index: instruction_index,
        eth_address_offset: eth_offset,
        eth_address_instruction_index: instruction_index,
        message_data_offset: msg_offset,
        message_data_size: message_len,
        message_instruction_index: instruction_index,
    };

    let bin_offsets = bincode::serialize(&offsets).unwrap();

    let mut instruction_data = Vec::with_capacity(1 + bin_offsets.len());
    instruction_data.push(NUMBER_OF_SIGNATURES);
    instruction_data.extend(&bin_offsets);

    instruction_data
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

fn parse_program_args() -> (Pubkey, String, String, String, String){
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
                .default_value("AeTXaphqg264q2Bf1iqnLMWNiyykrfK13cKDR6WBHLGY")
                // .default_value("Bn5MgusJdV4dhZYrTMXCDUNUfD69SyJLSXWwRk8sdp3x")
                .help("Pubkey for evm_loader contract")
        ).arg(
        Arg::with_name("transactions_file")
            .value_name("TRANSACTIONS_FILEPATH")
            .takes_value(true)
            .required(true)
            .help("/path/to/transactions.json")
            .default_value("/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/transactions.json1"),
    ).arg(
        Arg::with_name("senders_file")
            .value_name("SENDERS_FILEPATH")
            .takes_value(true)
            .required(true)
            .help("/path/to/senders.json")
            .default_value("/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/senders.json1"),
    )
        .arg(
            Arg::with_name("verify_file")
                .value_name("VERIFY_FILEPATH")
                .takes_value(true)
                .required(true)
                .help("/path/to/verify.json")
                .default_value("/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/verify.json1"),
        )        .get_matches();

    let evm_loader = pubkey_of(&app_matches, "evm_loader")
        .unwrap_or_else(|| {
            println!("Need specify evm_loader");
            exit(1);
        });


    let json_rpc_url = normalize_to_url_if_moniker(
        app_matches
            .value_of("json_rpc_url").unwrap()
    );

    let trx_filename = app_matches.value_of("transactions_file").unwrap().to_string();
    let senders_filename = app_matches.value_of("senders_file").unwrap().to_string();
    let verify_filename = app_matches.value_of("verify_file").unwrap().to_string();

    return (evm_loader, json_rpc_url, trx_filename, senders_filename, verify_filename);
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

fn create_trx(
    evm_loader: &Pubkey,
    trx_filename: &String,
    senders_filename :&String,
    rpc_client: &Rc<RpcClient> )-> Result<Vec<(Transaction, String, String, String)>, Error>{

    let keccakprog = Pubkey::from_str("KeccakSecp256k11111111111111111111111111111").unwrap();

    let mut keys = read_senders(&senders_filename).unwrap();

    println!("creating transactions  ..");
    let mut transaction = Vec::new();
    let mut it = keys.iter();

    let mut file = File::open(trx_filename)?;
    let reader= BufReader::new(file);

    for line in reader.lines(){

        let mut keypair_bin : &Vec<u8>;
        match (it.next()){
            Some(val) => keypair_bin = val,
            None => {
                it = keys.iter();
                keypair_bin = it.next().unwrap()
            }
        }
        let keypair =Keypair::from_bytes(keypair_bin).unwrap();
        let trx : trx_t = serde_json::from_str(line?.as_str())?;
        let msg = hex::decode(&trx.msg).unwrap();

        let data_keccak = make_keccak_instruction_data(1, msg.len() as u16, 1);
        // let data_keccak = make_secp256k1_instruction(1, msg.len() as u16, 1);
        let instruction_keccak = Instruction::new_with_bytes(
            keccakprog,
            &data_keccak,
            vec![
                AccountMeta::new_readonly(keccakprog, false),
            ]
        );

        let mut data_05_hex = String::from("05");
        data_05_hex.push_str(trx.from_addr.as_str());
        data_05_hex.push_str(trx.sign.as_str());
        data_05_hex.push_str(trx.msg.as_str());
        let data_05 : Vec<u8> = hex::decode(data_05_hex.as_str()).unwrap();

        let contract = Pubkey::from_str(trx.erc20_sol.as_str()).unwrap();
        let contract_code = Pubkey::from_str(trx.erc20_code.as_str()).unwrap();
        let caller = Pubkey::from_str(trx.payer_sol.as_str()).unwrap();
        let sysinstruct = Pubkey::from_str("Sysvar1nstructions1111111111111111111111111").unwrap();
        let sysvarclock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();

        let instruction_05 = Instruction::new_with_bytes(
            *evm_loader,
            &data_05,
            vec![
                AccountMeta::new(contract, false),
                AccountMeta::new(contract_code, false),
                AccountMeta::new(caller, false),
                AccountMeta::new_readonly(sysinstruct, false),
                AccountMeta::new_readonly(*evm_loader, false),
                AccountMeta::new_readonly(sysvarclock, false),
            ]);

        let message = Message::new(&[instruction_keccak, instruction_05], Some(&keypair.pubkey()));
        let mut tx = Transaction::new_unsigned(message);

        let blockhash : solana_program::hash::Hash;
        match (rpc_client.get_recent_blockhash()){
            Ok((hash,_)) => blockhash = hash,
            _ => panic!("get_recent_blockhash() error")
        }

        let signer: Box<dyn Signer> = Box::from(keypair);
        tx.try_sign(&[&*signer] , blockhash)?;
        transaction.push((tx, trx.erc20_eth, trx.payer_eth, trx.receiver_eth));
    }

    return Ok(transaction);
}

fn main() -> CommandResult{

    let (evm_loader, json_rpc_url,trx_filename, senders_filename, verify_filename )
        = parse_program_args();

    let rpc_client = Rc::new(RpcClient::new_with_commitment(json_rpc_url,
                                                            CommitmentConfig::confirmed()));

    let transaction = create_trx(&evm_loader, &trx_filename, &senders_filename, &rpc_client).unwrap();

    println!("sending transactions ..");
    let start = SystemTime::now();
    let mut count = 0;
    let mut signatures = Vec::new();
    for (tx, erc20_eth, payer_eth, receiver_eth) in transaction{
        let sig = rpc_client.send_transaction_with_config(
            &tx,
            RpcSendTransactionConfig {
                skip_preflight : true,
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                ..RpcSendTransactionConfig::default()
            }
        )?;
        signatures.push((erc20_eth, payer_eth, receiver_eth, sig));
        count = count  + 1;
    }
    let end = SystemTime::now();
    let time = end.duration_since(start).expect("Clock may have gone backwards");;
    println!("time  {:?}", time);
    println!("count {}", &count.to_string());

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

    Ok(())
}

