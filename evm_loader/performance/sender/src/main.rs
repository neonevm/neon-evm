// #![deny(warnings)]
// #![deny(clippy::all, clippy::pedantic, clippy::nursery)]
// #![allow(
// clippy::redundant_field_names,
// clippy::must_use_candidate,
// clippy::missing_errors_doc,
// clippy::missing_panics_doc,
// clippy::missing_const_for_fn
// )]

use std::fs::File;
use std::vec::Vec;

use std::{
//     collections::HashMap,
//     io::{Read},
//     fs::File,
//     env, str::FromStr,
//     process::exit,
    rc::Rc,
    // sync::Arc,
//     thread::sleep,
//     time::{Duration},
    i64,
    str::FromStr,
    process::exit,
};

use std::io::{self, prelude::*, BufReader};

use serde::{Deserialize, Serialize};
// use serde_json::Result;

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
use std::borrow::Borrow;

use evm_loader::{
    instruction::EvmInstruction,
    solana_backend::SolanaBackend,
    account_data::{AccountData, Account, Contract},
};
use evm::{H160, H256, U256};
use solana_sdk::recent_blockhashes_account::update_account;
// use

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


// fn main() -> std::io::Result<()>{
fn main() -> CommandResult{
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
                .default_value("jmHKzUejhejY2b212jTfy7fFbVfGbKchd3uHv9khT1A")
                .help("Pubkey for evm_loader contract")
        )
        .get_matches();

    let evm_loader = pubkey_of(&app_matches, "evm_loader")
        .unwrap_or_else(|| {
            println!("Need specify evm_loader");
            exit(1);
        });

    let json_rpc_url = normalize_to_url_if_moniker(
        app_matches
            .value_of("json_rpc_url").unwrap()
    );
    let rpc_client = Rc::new(RpcClient::new_with_commitment(json_rpc_url,
                                                            CommitmentConfig::confirmed()));

    let blockhash : solana_program::hash::Hash;
    match (rpc_client.get_recent_blockhash()){
        Ok((hash,_)) => blockhash = hash,
        _ => {panic!("get_recent_blockhash() error")}
    }
    println!("recent_block_hash {}", blockhash.to_string());
    let keccakprog = Pubkey::from_str("KeccakSecp256k11111111111111111111111111111").unwrap();
    let trx_file_name : &str = "/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/transactions.json1";
    // let sender_file_name : &str = "/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/senders.json1";

    // let mut file = File::open(sender_file_name)?;
    // let reader= BufReader::new(file);
    // let mut senders = Vec::new();
    // for line in reader.lines(){
    //     let sender : sender_t = serde_json::from_str(line?.as_str())?;
    //     println!("{}", sender.pr_key);
    //     let prkey : Vec<u8> = hex::decode(sender.pr_key).unwrap();
    //     let pubkey : Pubkey = Pubkey::from_str(sender.pub_key.as_str()).unwrap();
    //     senders.push((prkey,  pubkey));
    // }
    let mut file = File::open(trx_file_name)?;
    let reader= BufReader::new(file);
    // let mut count: usize = 0;
    // let mut iter = senders.iter();
    // let signer : Box<dyn Signer>;

    for line in reader.lines(){

        // let mut pub_key : Pubkey;
        // let mut pr_key = Vec::new() ;
        // let mut pr_key1 = Vec::new() ;
        // pr_key1.resize(32, 0 as u8);

        // match (iter.next()){
        //     None => {iter = senders.iter(); continue;},
        //     Some((pr_bin, pb)) => {
        //         for &i in pr_bin{
        //             pr_key.push(i);
        //         }
        //         pub_key = pb.clone();
        //     }
        // }
        // for &i in pr_key.iter().rev(){
        //     pr_key1.push(i);
        // }
        // println!("{}", &hex::encode(&pr_key1));
        // println!("{}", &hex::encode(&pr_key));
        // let keypair = Keypair::from_bytes(&pr_key1).unwrap();
        let keypair = Keypair::new();
        println!("wallet {}", keypair.pubkey());

        match (rpc_client.request_airdrop(&keypair.pubkey(), 1000)){
            Ok((signature)) => println! ("airdrop sig {}", &signature.to_string()),
            _ => {panic!("request_airdrop() error")}
        }

        let trx : trx_t = serde_json::from_str(line?.as_str())?;
        println!("{}",trx.erc20_code);
        println!("{}",trx.msg);
        let msg = hex::decode(&trx.msg).unwrap();
        let data_keccak = make_keccak_instruction_data(1, msg.len() as u16, 1);
        let instruction_keccak = Instruction::new_with_bincode(
            keccakprog,
            &data_keccak,
            vec![AccountMeta::new_readonly(keccakprog, false)]);

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

        let instruction_05 = Instruction::new_with_bincode(
            evm_loader,
            &data_05,
            vec![
                AccountMeta::new_readonly(contract, false),
                AccountMeta::new_readonly(contract_code, false),
                AccountMeta::new_readonly(caller, false),
                AccountMeta::new_readonly(sysinstruct, false),
                AccountMeta::new_readonly(evm_loader, false),
                AccountMeta::new_readonly(sysvarclock, false),
            ]);

        let message = Message::new(&[instruction_keccak, instruction_05], Some(&keypair.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        let signer: Box<dyn Signer> = Box::from(keypair);

        tx.try_sign(&[&*signer] , blockhash)?;
        println!("signed: {:x?}", tx);
        let sig = rpc_client.send_transaction(&tx)?;
        println!("sended: {:x?}", sig);

        // count = count + 1;
        // if count == senders.len(){
        //     count = 0;
        // }

    }
    Ok(())
}

// fn new_throwaway_signer() -> (Option<Box<dyn Signer>>, Option<Pubkey>) {
//     let keypair = Keypair::new();
//     let pubkey = keypair.pubkey();
//     (Some(Box::new(keypair) as Box<dyn Signer>), Some(pubkey))
// }

// let msg = i64::/from_str_radix(trx.msg.len(), 16).unwrap();
// let msg = i64::from//_str_radix(trx.msg.len(), 16).unwrap();
