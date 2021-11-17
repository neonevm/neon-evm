mod cmd_arg;
mod eth_transaction;
mod sol_transaction;

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
    keccak::Hasher,
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
struct sender_t{
    pub_key: String,
    pr_key: String,
}



type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;


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

fn read_collateral(filename: &String) -> Result<Vec<sol_transaction::collateral_t>, Error> {
    let mut file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut pool = Vec::new();

    for line in reader.lines() {
        let data: sol_transaction::collateral_t = serde_json::from_str(line?.as_str())?;
        pool.push(data);
    }
    return Ok(pool);
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

    let blockhash : solana_program::hash::Hash;
    match (rpc_client.get_recent_blockhash()){
        Ok((hash,_)) => blockhash = hash,
        _ => panic!("get_recent_blockhash() error")
    }

    for line in trx_reader.lines(){

        let mut keypair_bin : &Vec<u8>;
        match (it_keys.next()){
            Some(val) => keypair_bin = val,
            None => {
                it_keys = keys.iter();
                keypair_bin = it_keys.next().unwrap()
            }
        }

        let mut collateral_data : &sol_transaction::collateral_t;
        match (it_collaterals.next()){
            Some(val) => collateral_data = val,
            None => {
                it_collaterals = collaterals.iter();
                collateral_data = it_collaterals.next().unwrap()
            }
        }

        let keypair =Keypair::from_bytes(keypair_bin).unwrap();
        let trx : sol_transaction::trx_t = serde_json::from_str(line?.as_str())?;
        let msg = hex::decode(&trx.msg).unwrap();

        let data_keccak = sol_transaction::make_keccak_instruction_data(1, msg.len() as u16, 5);
        let instruction_keccak = Instruction::new_with_bytes(
            keccakprog,
            &data_keccak,
            vec![
                AccountMeta::new_readonly(keccakprog, false),
            ]
        );
        let keypair_pubkey = keypair.pubkey();
        let signer: Box<dyn Signer> = Box::from(keypair);
        let instruction_05 = sol_transaction::make_instruction_05(&trx, evm_loader, &signer.pubkey(), collateral_data);
        // let instruction_budget_units = make_instruction_budget_units();
        // let instruction_budget_heap = make_instruction_budget_heap();


        let message = Message::new(
            // &[instruction_budget_units, instruction_budget_heap, instruction_keccak, instruction_05],
            &[instruction_keccak, instruction_05],
            Some(&keypair_pubkey)
        );
        let mut tx = Transaction::new_unsigned(message);

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
        = cmd_arg::parse_program_args();

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

