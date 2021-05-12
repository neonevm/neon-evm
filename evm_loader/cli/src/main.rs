mod account_storage;
use crate::account_storage::EmulatorAccountStorage;

use evm_loader::{
    instruction::EvmInstruction,
    solana_backend::SolanaBackend,
    account_data::{AccountData, Account, Contract},
};

use evm::{executor::StackExecutor, ExitReason};
use bincode::serialize;
use hex;
use primitive_types::{H160, H256, U256};
use solana_sdk::{
    clock::Slot,
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    loader_instruction::LoaderInstruction,
    message::Message,
    pubkey::Pubkey,
    signature::Signer,
    signers::Signers,
    transaction::Transaction,
    system_program,
    system_instruction,
    sysvar::{rent, clock},
};
use serde_json::json;
use std::{
    cmp::min,
    collections::HashMap,
    io::{Read},
    fs::File,
    env, str::FromStr,
    net::{SocketAddr, UdpSocket},
    process::exit,
    thread::sleep,
    time::{Duration, Instant},
};

use clap::{
    crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker},
    keypair::{signer_from_path},
};

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
    rpc_request::MAX_GET_SIGNATURE_STATUSES_QUERY_ITEMS,
    rpc_response::RpcLeaderSchedule,
};
use solana_cli::{
    checks::{check_account_for_fee, check_account_for_spend_multiple_fees_with_commitment},
    send_tpu::{get_leader_tpus, send_transaction_tpu},
};
use solana_cli_output::display::new_spinner_progress_bar;
use solana_transaction_status::TransactionConfirmationStatus;

use sha3::{Keccak256, Digest};

use log::*;

const DATA_CHUNK_SIZE: usize = 229; // Keep program chunks under PACKET_DATA_SIZE
const NUM_TPU_LEADERS: u64 = 2;

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

pub struct Config {
    rpc_client: RpcClient,
    evm_loader: Pubkey,
    fee_payer: Pubkey,
    signer: Box<dyn Signer>,
}

fn command_emulate(config: &Config, contract_id: H160, caller_id: H160, data: Vec<u8>) -> CommandResult {
    let account_storage = EmulatorAccountStorage::new(config, contract_id, caller_id);

    let (exit_reason, result, applies_logs) = {
        let backend = SolanaBackend::new(&account_storage, None);
        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
    
        let (exit_reason, result) = executor.transact_call(caller_id, contract_id, U256::zero(), data, usize::max_value());
    
        debug!("Call done");
        
        if exit_reason.is_succeed() {
            debug!("Succeed execution");
            let (applies, logs) = executor.deconstruct();
            (exit_reason, result, Some((applies, logs)))
        } else {
            (exit_reason, result, None)
        }
    };

    debug!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, _logs) = applies_logs.unwrap();
    
            account_storage.apply(applies);

            debug!("Applies done");
            "succeed".to_string()
        }
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
    };

    info!("{}", &status);
    info!("{}", &hex::encode(&result));

    if !exit_reason.is_succeed() {
        debug!("Not succeed execution");
    }

    account_storage.get_used_accounts(&status, &result);

    Ok(())
}

fn command_create_program_address (
    config: &Config,
    seed: &str,
) -> CommandResult {
    let strings = seed.split_whitespace().collect::<Vec<_>>();
    let mut seeds = vec![];
    let mut seeds_vec = vec![];
    for s in strings {
        seeds_vec.push(hex::decode(s).unwrap());
    }
    for i in &seeds_vec {seeds.push(&i[..]);}
    let (address,nonce) = Pubkey::find_program_address(&seeds, &config.evm_loader);
    println!("{} {}", address, nonce);

    Ok(())
}

fn command_create_ether_account (
    config: &Config,
    ether_address: &H160,
    lamports: u64,
    space: u64
) -> CommandResult {
    let (solana_address, nonce) = Pubkey::find_program_address(&[ether_address.as_bytes()], &config.evm_loader);
    debug!("Create ethereum account {} <- {} {}", solana_address, hex::encode(ether_address), nonce);

    let instruction = Instruction::new(
            config.evm_loader,
            &EvmInstruction::CreateAccount {lamports, space, ether: *ether_address, nonce},
            vec![
                AccountMeta::new(config.signer.pubkey(), true),
                AccountMeta::new(solana_address, false),
                AccountMeta::new_readonly(system_program::id(), false)
            ]);

    let finalize_message = Message::new(&[instruction], Some(&config.signer.pubkey()));
    let (blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;

    check_account_for_fee(
        &config.rpc_client,
        &config.signer.pubkey(),
        &fee_calculator,
        &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*config.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    config.rpc_client.send_and_confirm_transaction_with_spinner(&finalize_tx)?;

    println!("{}", json!({
        "solana": format!("{}", solana_address),
        "ether": format!("{}", hex::encode(ether_address)),
        "nonce": nonce,
    }).to_string());
    Ok(())
}

fn read_program_data(program_location: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = File::open(program_location).map_err(|err| {
        format!("Unable to open program file: {}", err)
    })?;
    let mut program_data = Vec::new();
    file.read_to_end(&mut program_data).map_err(|err| {
        format!("Unable to read program file: {}", err)
    })?;

    Ok(program_data)
}

fn send_and_confirm_transactions_with_spinner<T: Signers>(
    rpc_client: &RpcClient,
    mut transactions: Vec<Transaction>,
    signer_keys: &T,
    commitment: CommitmentConfig,
    mut last_valid_slot: Slot,
) -> CommandResult {
    let progress_bar = new_spinner_progress_bar();
    let mut send_retries = 5;
    let mut leader_schedule: Option<RpcLeaderSchedule> = None;
    let mut leader_schedule_epoch = 0;
    let send_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    let cluster_nodes = rpc_client.get_cluster_nodes().ok();

    loop {
        progress_bar.set_message("Finding leader nodes...");
        let epoch_info = rpc_client.get_epoch_info()?;
        let mut slot = epoch_info.absolute_slot;
        let mut last_epoch_fetch = Instant::now();
        if epoch_info.epoch > leader_schedule_epoch || leader_schedule.is_none() {
            leader_schedule = rpc_client.get_leader_schedule(Some(epoch_info.absolute_slot))?;
            leader_schedule_epoch = epoch_info.epoch;
        }

        let mut tpu_addresses = get_leader_tpus(
            min(epoch_info.slot_index + 1, epoch_info.slots_in_epoch),
            NUM_TPU_LEADERS,
            leader_schedule.as_ref(),
            cluster_nodes.as_ref(),
        );

        // Send all transactions
        let mut pending_transactions = HashMap::new();
        let num_transactions = transactions.len();
        for transaction in transactions {
            if !tpu_addresses.is_empty() {
                let wire_transaction =
                    serialize(&transaction).expect("serialization should succeed");
                for tpu_address in &tpu_addresses {
                    send_transaction_tpu(&send_socket, &tpu_address, &wire_transaction);
                }
            } else {
                let _result = rpc_client
                    .send_transaction_with_config(
                        &transaction,
                        RpcSendTransactionConfig {
                            preflight_commitment: Some(commitment.commitment),
                            ..RpcSendTransactionConfig::default()
                        },
                    )
                    .ok();
            }
            pending_transactions.insert(transaction.signatures[0], transaction);
            progress_bar.set_message(&format!(
                "[{}/{}] Transactions sent",
                pending_transactions.len(),
                num_transactions
            ));

            // Throttle transactions to about 100 TPS
            sleep(Duration::from_millis(10));

            // Update leader periodically
            if last_epoch_fetch.elapsed() > Duration::from_millis(400) {
                let epoch_info = rpc_client.get_epoch_info()?;
                last_epoch_fetch = Instant::now();
                tpu_addresses = get_leader_tpus(
                    min(epoch_info.slot_index + 1, epoch_info.slots_in_epoch),
                    NUM_TPU_LEADERS,
                    leader_schedule.as_ref(),
                    cluster_nodes.as_ref(),
                );
            }
        }

        // Collect statuses for all the transactions, drop those that are confirmed
        loop {
            let pending_signatures = pending_transactions.keys().cloned().collect::<Vec<_>>();
            for pending_signatures_chunk in
                pending_signatures.chunks(MAX_GET_SIGNATURE_STATUSES_QUERY_ITEMS)
            {
                if let Ok(result) = rpc_client.get_signature_statuses(pending_signatures_chunk) {
                    let statuses = result.value;
                    for (signature, status) in
                        pending_signatures_chunk.iter().zip(statuses.into_iter())
                    {
                        if let Some(status) = status {
                            if let Some(confirmation_status) = &status.confirmation_status {
                                if *confirmation_status != TransactionConfirmationStatus::Processed
                                {
                                    let _ = pending_transactions.remove(signature);
                                }
                            } else if status.confirmations.is_none()
                                || status.confirmations.unwrap() > 1
                            {
                                let _ = pending_transactions.remove(signature);
                            }
                        }
                    }
                }

                slot = rpc_client.get_slot()?;
                progress_bar.set_message(&format!(
                    "[{}/{}] Transactions confirmed. Retrying in {} slots",
                    num_transactions - pending_transactions.len(),
                    num_transactions,
                    last_valid_slot.saturating_sub(slot)
                ));
            }

            if pending_transactions.is_empty() {
                return Ok(());
            }

            if slot > last_valid_slot {
                break;
            }

            let epoch_info = rpc_client.get_epoch_info()?;
            tpu_addresses = get_leader_tpus(
                min(epoch_info.slot_index + 1, epoch_info.slots_in_epoch),
                NUM_TPU_LEADERS,
                leader_schedule.as_ref(),
                cluster_nodes.as_ref(),
            );

            for transaction in pending_transactions.values() {
                if !tpu_addresses.is_empty() {
                    let wire_transaction =
                        serialize(&transaction).expect("serialization should succeed");
                    for tpu_address in &tpu_addresses {
                        send_transaction_tpu(&send_socket, &tpu_address, &wire_transaction);
                    }
                } else {
                    let _result = rpc_client
                        .send_transaction_with_config(
                            transaction,
                            RpcSendTransactionConfig {
                                preflight_commitment: Some(commitment.commitment),
                                ..RpcSendTransactionConfig::default()
                            },
                        )
                        .ok();
                }
            }

            if cfg!(not(test)) {
                // Retry twice a second
                sleep(Duration::from_millis(500));
            }
        }

        if send_retries == 0 {
            return Err("Transactions failed".into());
        }
        send_retries -= 1;

        // Re-sign any failed transactions with a new blockhash and retry
        let (blockhash, _fee_calculator, new_last_valid_slot) = rpc_client
            .get_recent_blockhash_with_commitment(commitment)?
            .value;
        last_valid_slot = new_last_valid_slot;
        transactions = vec![];
        for (_, mut transaction) in pending_transactions.into_iter() {
            transaction.try_sign(signer_keys, blockhash)?;
            transactions.push(transaction);
        }
    }
}

fn command_deploy(
    config: &Config,
    program_location: &str,
) -> CommandResult {

    let ACCOUNT_HEADER_SIZE = 1+Account::SIZE;
    let CONTRACT_HEADER_SIZE = 1+Contract::SIZE;

    let program_data = read_program_data(program_location)?;
    let program_code_len = CONTRACT_HEADER_SIZE + program_data.len() + 2*1024;
    let minimum_balance_for_account = config.rpc_client.get_minimum_balance_for_rent_exemption(ACCOUNT_HEADER_SIZE)?;
    let minimum_balance_for_code = config.rpc_client.get_minimum_balance_for_rent_exemption(program_code_len)?;

    let creator = &config.signer;
    let signers = [&*config.signer];

    let creator_ether: H160 = H256::from_slice(Keccak256::digest(&creator.pubkey().to_bytes()).as_slice()).into();
    debug!("Creator: ether {}, solana {}", creator_ether, creator.pubkey());

    let (program_id, ether, nonce) = {
        let code_hash = Keccak256::digest(&program_data);
        let mut hasher = Keccak256::new();
        hasher.input(&[0xff]);
        hasher.input(&creator_ether.as_bytes());
        hasher.input(&[0u8; 32]);
        hasher.input(&code_hash.as_slice());
        let ether: H160 = H256::from_slice(hasher.result().as_slice()).into();
        let seeds = [ether.as_bytes()];
        let (address, nonce) = Pubkey::find_program_address(&seeds[..], &config.evm_loader);
        debug!("Creator: {}, code_hash: {}", &hex::encode(&creator.pubkey().to_bytes()), &hex::encode(code_hash.as_slice()));
        (address, ether, nonce)
    };

    debug!("Create account: {} with {} {}", program_id, ether, nonce);  

    let (program_code, program_seed) = {
        let seed = bs58::encode(&ether.to_fixed_bytes()).into_string();
        debug!("Code account seed {} and len {}", &seed, &seed.len());
        let address = Pubkey::create_with_seed(&creator.pubkey(), &seed, &config.evm_loader).unwrap();
        (address, seed)
    };

    debug!("Create code account: {}", &program_code.to_string());

    let make_create_account_instruction = |acc: &Pubkey, ether: &H160, nonce: u8, balance: u64| {
        Instruction::new(
            config.evm_loader,
            &(2u32, balance, 0 as u64, ether.as_fixed_bytes(), nonce),
            vec![AccountMeta::new(creator.pubkey(), true),
                 AccountMeta::new(*acc, false),
                 AccountMeta::new(program_code, false),
                 AccountMeta::new_readonly(system_program::id(), false),]
        )
    };

    let make_write_instruction = |offset: u32, bytes: Vec<u8>| -> Instruction {
        Instruction::new(
            config.evm_loader,
            &LoaderInstruction::Write {offset, bytes},
            vec![AccountMeta::new(program_code, false),
                 AccountMeta::new(creator.pubkey(), true)]
        )
    };

    let make_finalize_instruction = || -> Instruction {
        Instruction::new(
            config.evm_loader,
            &LoaderInstruction::Finalize,
            vec![AccountMeta::new(program_id, false),
                 AccountMeta::new(program_code, false),
                //  AccountMeta::new(caller_id, false),
                 AccountMeta::new(creator.pubkey(), true),
                 AccountMeta::new(clock::id(), false),
                 AccountMeta::new(rent::id(), false),
                 AccountMeta::new(config.evm_loader, false),
                ]
        )
    };


    // Check program account to see if partial initialization has occurred
    let initial_instructions = if let Some(account) = config.rpc_client
        .get_account_with_commitment(&program_id, config.rpc_client.commitment())?
        .value
    {
        return Err(format!("Account already exist").into());
    } else {
        let mut instructions = Vec::new();
        // if let Some(account) = config.rpc_client.get_account_with_commitment(&caller_id, config.commitment)?.value {
        //     // TODO Check caller account
        // } else {
        //     instructions.push(make_create_account_instruction(&caller_id, &caller_ether, caller_nonce, minimum_balance_for_account, 0));
        // }
        instructions.push(system_instruction::create_account_with_seed(&creator.pubkey(), &program_code, &creator.pubkey(), &program_seed, minimum_balance_for_code, program_code_len as u64, &config.evm_loader));
        instructions.push(make_create_account_instruction(&program_id, &ether, nonce, minimum_balance_for_account));
        instructions
    };
    let balance_needed = minimum_balance_for_account + minimum_balance_for_code;
    debug!("Minimum balance: {}", balance_needed);

    //debug!("Initialize instructions: {:x?}", initial_instructions);  

    let initial_message = Message::new(&initial_instructions, Some(&config.signer.pubkey()));
    let mut messages: Vec<&Message> = Vec::new();
    messages.push(&initial_message);

    let mut write_messages = vec![];

    let mut code_len = Vec::new();
    code_len.extend_from_slice(&(program_data.len() as u64).to_le_bytes());
    let message = Message::new(&[make_write_instruction(0u32, code_len)], Some(&creator.pubkey()));
    write_messages.push(message);

    // Write code
    for (chunk, i) in program_data.chunks(DATA_CHUNK_SIZE).zip(0..) {
        let message = Message::new(&[make_write_instruction((8+i*DATA_CHUNK_SIZE) as u32, chunk.to_vec())], Some(&creator.pubkey()));
        write_messages.push(message);
    }
    let mut write_message_refs = vec![];
    for message in write_messages.iter() {write_message_refs.push(message);}
    messages.append(&mut write_message_refs);

    let finalize_message = Message::new(&[make_finalize_instruction()], Some(&creator.pubkey()));
    messages.push(&finalize_message);

    let (blockhash, fee_calculator, _) = config.rpc_client
        .get_recent_blockhash_with_commitment(config.rpc_client.commitment())?
        .value;

    check_account_for_spend_multiple_fees_with_commitment(
        &config.rpc_client,
        &config.signer.pubkey(),
        balance_needed,
        &fee_calculator,
        &messages,
        config.rpc_client.commitment(),
    )?;

    {  // Send initialize message
        debug!("Creating or modifying program account");
        let mut initial_transaction = Transaction::new_unsigned(initial_message);
        initial_transaction.try_sign(&signers, blockhash)?;
        config.rpc_client.send_and_confirm_transaction_with_spinner_and_config(
            &initial_transaction,
            config.rpc_client.commitment(),
            RpcSendTransactionConfig::default()
        )?;
    }

    {  // Send write message
        let (blockhash, _, last_valid_slot) = config.rpc_client
            .get_recent_blockhash_with_commitment(config.rpc_client.commitment())?
            .value;
    
        let mut write_transactions = vec![];
        for message in write_messages.into_iter() {
            let mut tx = Transaction::new_unsigned(message);
            tx.try_sign(&signers, blockhash)?;
            write_transactions.push(tx);
        }
    
        debug!("Writing program data");
        send_and_confirm_transactions_with_spinner(
            &config.rpc_client,
            write_transactions,
            &signers,
            config.rpc_client.commitment(),
            last_valid_slot,
        ).map_err(|err| {
            format!("Data writes to program account failed: {}", err)
        })?;
        debug!("Writing program data done");
    }

    { // Send finalize message
        let (blockhash, _, _) = config.rpc_client
            .get_recent_blockhash_with_commitment(config.rpc_client.commitment())?
            .value;
        let mut finalize_tx = Transaction::new_unsigned(finalize_message);
        finalize_tx.try_sign(&signers, blockhash)?;
    
        debug!("Finalizing program account");
        config.rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &finalize_tx,
                config.rpc_client.commitment(),
                RpcSendTransactionConfig {
                    skip_preflight: true,
                    ..RpcSendTransactionConfig::default()
                },
            ).map_err(|e| {
                format!("Finalizing program account failed: {}", e)
            })?;
    }

    println!("{}", json!({
        "programId": format!("{}", program_id),
        "codeId": format!("{}", program_code),
        "ethereum": format!("{:?}", ether),
    }).to_string());
    Ok(())
}

fn command_get_ether_account_data (
    config: &Config,
    ether_address: &H160,
) -> CommandResult {
    match EmulatorAccountStorage::get_account_from_solana(&config, ether_address) {
        Some((acc, code_account)) => {
            let solana_address =  Pubkey::find_program_address(&[&ether_address.to_fixed_bytes()], &config.evm_loader).0;
            let account_data = AccountData::unpack(&acc.data).unwrap();
            let account_data = AccountData::get_account(&account_data).unwrap();

            println!("Ethereum address: 0x{}", &hex::encode(&ether_address.as_fixed_bytes()));
            println!("Solana address: {}", solana_address);
    
            println!("Account fields");
            println!("    ether: {}", &account_data.ether);
            println!("    nonce: {}", &account_data.nonce);
            println!("    trx_count: {}", &account_data.trx_count);
            println!("    signer: {}", &account_data.signer);
            println!("    code_account: {}", &account_data.code_account);
            println!("    blocked: {}", &account_data.blocked.is_some());
        
            if let Some(code_account) = code_account {
                let code_data = AccountData::unpack(&code_account.data).unwrap();
                let header = AccountData::size(&code_data);
                let code_data = AccountData::get_contract(&code_data).unwrap();

                println!("Contract fields");
                println!("    owner: {}", &code_data.owner);
                println!("    code_size: {}", &code_data.code_size);
                println!("    code as hex:");
    
                let code_size = code_data.code_size;
                let mut offset = header;
                while offset < ( code_size as usize + header) {
                    let data_slice = &code_account.data.as_slice();
                    let remains = if code_size as usize + header - offset > 80 {
                        80
                    } else {
                        code_size as usize + header - offset
                    };
    
                    println!("        {}", &hex::encode(&data_slice[offset+header..offset+header+remains]));
                    offset += remains;
                }
            }


        },
        None => {
            eprintln!("Account not found {}", &ether_address.to_string());
        }
    }

    Ok(())
}

fn make_clean_hex<'a>(in_str: &'a str) -> &'a str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {        
        &in_str
    }
}

// Return H160 for an argument
fn h160_of(matches: &ArgMatches<'_>, name: &str) -> Option<H160> {
    matches.value_of(name).map(|value| {
        H160::from_str(&make_clean_hex(value)).unwrap()
    })
}

// Return an error if string cannot be parsed as a H160 address
fn is_valid_h160<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    H160::from_str(&make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

// Return hexdata for an argument
fn hexdata_of(matches: &ArgMatches<'_>, name: &str) -> Option<Vec<u8>> {
    matches.value_of(name).map(|value| {
        hex::decode(&make_clean_hex(value)).unwrap()
    })
}

// Return an error if string cannot be parsed as a hexdata
fn is_valid_hexdata<T>(string: T) -> Result<(), String> where T: AsRef<str>,
{
    hex::decode(&make_clean_hex(string.as_ref())).map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    let app_matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
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
                .default_value("max")
                .help("Return information at the selected commitment level [possible values: processed, confirmed, finalized]"),
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
                        .validator(is_valid_h160)
                        .help("The contract that executes the transaction")
                )
                .arg(
                    Arg::with_name("data")
                        .value_name("DATA")
                        .takes_value(true)
                        .index(3)
                        .required(true)
                        .validator(is_valid_hexdata)
                        .help("Transaction data")
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
                .arg(
                    Arg::with_name("lamports")
                        .long("lamports")
                        .value_name("lamports")
                        .takes_value(true)
                        .default_value("0")
                        .required(false)
                )
                .arg(
                    Arg::with_name("space")
                        .long("space")
                        .value_name("space")
                        .takes_value(true)
                        .required(false)
                        .default_value("0")
                        .help("Length of data for new account"),
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
        .get_matches();

        stderrlog::new()
            .module(module_path!())
            .verbosity(app_matches.occurrences_of("verbose") as usize)
            .init()
            .unwrap();

        let mut wallet_manager = None;
        let config = {
            let cli_config = if let Some(config_file) = app_matches.value_of("config_file") {
                solana_cli_config::Config::load(config_file).unwrap_or_default()
            } else {
                solana_cli_config::Config::default()
            };

            let commitment = CommitmentConfig::from_str(app_matches.value_of("commitment").unwrap()).unwrap();

            let json_rpc_url = normalize_to_url_if_moniker(
                app_matches
                    .value_of("json_rpc_url")
                    .unwrap_or(&cli_config.json_rpc_url),
            );

            let evm_loader = pubkey_of(&app_matches, "evm_loader")
                    .unwrap_or_else(|| {
                        error!("Need specify evm_loader");
                        exit(1);
                    });

            let (signer, fee_payer) = signer_from_path(
                &app_matches,
                app_matches
                    .value_of("fee_payer")
                    .unwrap_or(&cli_config.keypair_path),
                "fee_payer",
                &mut wallet_manager,
            )
            .map(|s| {
                let p = s.pubkey();
                (s, p)
            })
            .unwrap_or_else(|e| {
                error!("{}", e);
                exit(1);
            });

            Config {
                rpc_client: RpcClient::new_with_commitment(json_rpc_url, commitment),
                evm_loader,
                fee_payer,
                signer,
            }
        };

        let (sub_command, sub_matches) = app_matches.subcommand();
        let result = match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_of(&arg_matches, "contract").unwrap();
                let sender = h160_of(&arg_matches, "sender").unwrap();
                let data = hexdata_of(&arg_matches, "data").unwrap();

                command_emulate(&config, contract, sender, data)
            }
            ("create-program-address", Some(arg_matches)) => {
                let seed = arg_matches.value_of("seed").unwrap().to_string();

                command_create_program_address(&config, &seed)
            }
            ("create-ether-account", Some(arg_matches)) => {
                let ether = h160_of(&arg_matches, "ether").unwrap();
                let lamports = value_t_or_exit!(arg_matches, "lamports", u64);
                let space = value_t_or_exit!(arg_matches, "space", u64);

                command_create_ether_account(&config, &ether, lamports, space)
            }
            ("deploy", Some(arg_matches)) => {
                let program_location = arg_matches.value_of("program_location").unwrap().to_string();

                command_deploy(&config, &program_location)
            }
            ("get-ether-account-data", Some(arg_matches)) => {
                let ether = h160_of(&arg_matches, "ether").unwrap();

                command_get_ether_account_data(&config, &ether)
            }
            _ => unreachable!(),
        };
        match result {
            Ok(()) => exit(0),
            Err(err) => {
                error!("{}", err);
                exit(1);
            }
        }
}
