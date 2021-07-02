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
// use std::vec::Vec;

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
};

use std::io::{self, prelude::*, BufReader};

use serde::{Deserialize, Serialize};
use serde_json::Result;

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
fn main() -> std::io::Result<()>{
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
        .get_matches();

    let json_rpc_url = normalize_to_url_if_moniker(
        app_matches
            .value_of("json_rpc_url").unwrap()
    );

    let file_name : &str = "/home/user/CLionProjects/cyber-core/neon-evm/evm_loader/performance/transactions.json1";

    let mut file = File::open(file_name)?;
    let reader= BufReader::new(file);
    for line in reader.lines(){
        let trx : trx_t = serde_json::from_str(line?.as_str())?;
        println!("{}",trx.erc20_code);
    }

    let rpc_client = Rc::new(RpcClient::new_with_commitment(json_rpc_url,
                                                            CommitmentConfig::confirmed()));

    let blockhash : solana_program::hash::Hash;
    match (rpc_client.get_recent_blockhash()){
        Ok((hash,_)) => blockhash = hash,
        _ => {panic!("get_recent_blockhash() error")}
    }
    println!("recent_block_hash {}", blockhash.to_string());

    let keccakprog = "KeccakSecp256k11111111111111111111111111111";
    let prog_id = bs58::decode(keccakprog).into_vec().unwrap();
    let lamports :u64 = 1;
    let space : u64 = 1;
    let v = vec![0; 20];
    let ether : H160 = H160::from_slice(&v) ;
    let nonce : u8 =0;

    let instruction = Instruction::new_with_bincode(
        Pubkey::new(prog_id.as_ref()) ,
        &EvmInstruction::CreateAccount {lamports, space, ether, nonce},
        vec![
            // AccountMeta::new(config.signer.pubkey(), true),
            // AccountMeta::new(solana_address, false),
            AccountMeta::new_readonly(system_program::id(), false)
        ]);

    Ok(())
}
