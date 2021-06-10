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
use evm::{H160, H256, U256};
use solana_sdk::{
    clock::Slot,
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    loader_instruction::LoaderInstruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Signer, Signature},
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
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

use clap::{
    crate_description, crate_name, crate_version, value_t_or_exit, App, AppSettings, Arg,
    ArgMatches, SubCommand,
};

use solana_program::keccak::{hash, hashv};

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
    tpu_client::{TpuClient, TpuClientConfig},
};
use solana_cli::{
    checks::{check_account_for_fee, check_account_for_spend_multiple_fees_with_commitment},
};
use solana_cli_output::display::new_spinner_progress_bar;
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding, EncodedTransaction, UiMessage, UiInstruction};

use sha3::{Keccak256, Digest};

use rlp::RlpStream;

use log::*;

const DATA_CHUNK_SIZE: usize = 229; // Keep program chunks under PACKET_DATA_SIZE
const NUM_TPU_LEADERS: u64 = 2;

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

pub struct Config {
    rpc_client: Arc<RpcClient>,
    websocket_url: String,
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
    rpc_client: Arc<RpcClient>,
    websocket_url: &str,
    mut transactions: Vec<Transaction>,
    signer_keys: &T,
    commitment: CommitmentConfig,
    mut last_valid_slot: Slot,
) -> CommandResult {
    let progress_bar = new_spinner_progress_bar();
    let mut send_retries = 5;

    progress_bar.set_message("Finding leader nodes...");
    let tpu_client = TpuClient::new(
        rpc_client.clone(),
        websocket_url,
        TpuClientConfig::default(),
    )?;

    loop {
        // Send all transactions
        let mut pending_transactions = HashMap::new();
        let num_transactions = transactions.len();
        for transaction in transactions {
            if !tpu_client.send_transaction(&transaction) {
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
        }

        // Collect statuses for all the transactions, drop those that are confirmed
        loop {
            let mut slot = 0;
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

            for transaction in pending_transactions.values() {
                if !tpu_client.send_transaction(transaction) {
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

pub fn keccak256_h256(data: &[u8]) -> H256 {
    H256::from(hash(&data).to_bytes())
}

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    hash(&data).to_bytes()
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

fn create_account_with_seed(config: &Config, funding: &Pubkey, base: &Pubkey, seed: &str, len: u64) ->  Result<Pubkey, Error>
{
    let storage = Pubkey::create_with_seed(&base, &seed, &config.evm_loader).unwrap();

    if config.rpc_client.get_account_with_commitment(&storage, config.rpc_client.commitment())?.value.is_none() {
        debug!("Account not found");
        let create_acc_instruction = system_instruction::create_account_with_seed(&funding, &storage, &base, &seed, 10u64.pow(9), len, &config.evm_loader);
        send_transaction(config, &[create_acc_instruction])?;
    } else {
        debug!("Account found");
    }

    Ok(storage)
}

fn send_transaction(
    config: &Config,
    instructions: &[Instruction]) -> Result<Signature, Error>
{
    let message = Message::new(instructions, Some(&config.signer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    let signers = [&*config.signer];
    let (blockhash, _, _last_valid_slot) = config.rpc_client.get_recent_blockhash_with_commitment(config.rpc_client.commitment())?.value;
    transaction.try_sign(&signers, blockhash)?;
    Ok(config.rpc_client.send_and_confirm_transaction_with_spinner_and_config(
        &transaction,
        config.rpc_client.commitment(),
        RpcSendTransactionConfig::default()
    )?)
}

fn command_deploy(
    config: &Config,
    program_location: &str,
    _caller: Pubkey
) -> CommandResult {
    use secp256k1::{PublicKey, SecretKey};
    use ethereum_types::{Address, U256};

    let ACCOUNT_HEADER_SIZE = 1+Account::SIZE;
    let CONTRACT_HEADER_SIZE = 1+Contract::SIZE;

    let creator = &config.signer;
    let signers = [&*config.signer];
    let program_data = read_program_data(program_location)?;

    let program_code_len = CONTRACT_HEADER_SIZE + program_data.len() + 2*1024;
    let minimum_balance_for_account = config.rpc_client.get_minimum_balance_for_rent_exemption(ACCOUNT_HEADER_SIZE)?;
    let minimum_balance_for_code = config.rpc_client.get_minimum_balance_for_rent_exemption(program_code_len)?;

    let (caller_private, caller_ether, caller_sol, caller_nonce) = {
        let random_vec_32: [u8;32] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 1];
        let caller_private = SecretKey::parse(&random_vec_32)?;
        let caller_public = PublicKey::from_secret_key(&caller_private);
        let pk_data = caller_public.serialize();
        let sender = Keccak256::digest(&pk_data);
        let caller_ether = Address::from_slice(&sender[..20]);
        let (caller_sol, caller_nonce) = Pubkey::find_program_address(&[&caller_ether.to_fixed_bytes()], &config.evm_loader);
        debug!("caller_sol = {}", caller_sol);
        debug!("caller_ether = {}", caller_ether);
        (caller_private, caller_ether, caller_sol, caller_nonce)
    };

    // Create caller account if not exists
    if config.rpc_client.get_account_with_commitment(&caller_sol, config.rpc_client.commitment())?.value.is_none() {
        debug!("Caller account not found");
        let create_acc_instruction = Instruction::new_with_bincode(
            config.evm_loader,
            &(2u32, minimum_balance_for_account, 0 as u64, caller_ether.as_fixed_bytes(), caller_nonce),
            vec![AccountMeta::new(creator.pubkey(), true),
                 AccountMeta::new(caller_sol, false),
                 AccountMeta::new_readonly(system_program::id(), false),]
        );
        send_transaction(config, &[create_acc_instruction])?;
    } else {
        debug!("Caller account found");
    }

    let trx_count = {
        let data : Vec<u8>;
        match config.rpc_client.get_account_with_commitment(&caller_sol, CommitmentConfig::confirmed())?.value{
            Some(acc) =>   data = acc.data,
            _ => panic!("AccountNotFound: pubkey = {}", &caller_sol.to_string())
        }

        let trx_count : u64;
        let account = match evm_loader::account_data::AccountData::unpack(&data) {
            Ok(acc_data) =>
                match acc_data {
                AccountData::Account(acc) => acc,
                _ => return Err(format!("Caller has incorrect type").into())
            },
            Err(_) => return Err(format!("Caller unpack error").into())
        };
        trx_count = account.trx_count;
        debug!("trx_count = {}", trx_count);

        trx_count
    };

    let msg = {
        let rlp_data = {
            let tx = UnsignedTransaction {
                to: None,
                nonce: trx_count,
                gas_limit: 1.into(),
                gas_price: 1.into(),
                value: 0.into(),
                data: program_data,
                chain_id: 111.into(),
            };

            rlp::encode(&tx).to_vec()
        };

        let (sig, rec) = {
            use secp256k1::{Message, sign};
            let msg = Message::parse(&keccak256(rlp_data.as_slice()));
            sign(&msg, &caller_private)
        };

        let mut msg : Vec<u8> = vec!();
        msg.extend(sig.serialize().iter().copied());
        msg.push(rec.serialize());
        msg.extend((rlp_data.len() as u64).to_le_bytes().iter().copied());
        msg.extend(rlp_data);

        msg
    };

    let holder = create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &"1236".to_string(), 128*1024 as u64)?;

    let (program_id, program_ether, program_nonce) = {
        let trx_count_256 : U256 = U256::from(trx_count);
        let mut stream = rlp::RlpStream::new_list(2);
        stream.append(&caller_ether);
        stream.append(&trx_count_256);
        let ether : H160 = keccak256_h256(&stream.out()).into();
        let seeds = [ether.as_bytes()];
        let (address, nonce) = Pubkey::find_program_address(&seeds[..], &config.evm_loader);
        (address, ether, nonce)
    };

    debug!("Create account: {} with {} {}", program_id, program_ether, program_nonce);
    let (program_code, program_seed) = {
        let seed = bs58::encode(&program_ether.to_fixed_bytes()).into_string();
        debug!("Code account seed {} and len {}", &seed, &seed.len());
        let address = Pubkey::create_with_seed(&creator.pubkey(), &seed, &config.evm_loader).unwrap();
        (address, seed)
    };
    debug!("Create code account: {}", &program_code.to_string());

    let make_create_account_instruction = |acc: &Pubkey, ether: &H160, nonce: u8, balance: u64| {
        Instruction::new_with_bincode(
            config.evm_loader,
            &(2u32, balance, 0 as u64, ether.as_fixed_bytes(), nonce),
            vec![AccountMeta::new(creator.pubkey(), true),
                 AccountMeta::new(*acc, false),
                 AccountMeta::new(program_code, false),
                 AccountMeta::new_readonly(system_program::id(), false),]
        )
    };

    // Check program account to see if partial initialization has occurred
    if let Some(_account) = config.rpc_client
        .get_account_with_commitment(&program_id, config.rpc_client.commitment())?
        .value
    {
        // return Err(format!("Account already exist").into());
        debug!("Account already exist");
    } else {
        let mut instructions = Vec::new();
        instructions.push(system_instruction::create_account_with_seed(&creator.pubkey(), &program_code, &creator.pubkey(), &program_seed, minimum_balance_for_code, program_code_len as u64, &config.evm_loader));
        instructions.push(make_create_account_instruction(&program_id, &program_ether, program_nonce, minimum_balance_for_account));

        send_transaction(config, &instructions)?;
    };

    let make_write_instruction = |offset: u32, bytes: Vec<u8>| -> Instruction {
        Instruction::new_with_bincode(
            config.evm_loader,
            &LoaderInstruction::Write {offset, bytes},
            vec![AccountMeta::new(holder, false),
                 AccountMeta::new(creator.pubkey(), true)]
        )
    };

    let mut write_messages = vec![];
    // Write code
    debug!("Write code");
    for (chunk, i) in msg.chunks(DATA_CHUNK_SIZE).zip(0..) {
        let message = Message::new(&[make_write_instruction((i*DATA_CHUNK_SIZE) as u32, chunk.to_vec())], Some(&creator.pubkey()));
        write_messages.push(message);
    }
    debug!("Send write message");
    // Send write message
    {
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
            config.rpc_client.clone(),
            &config.websocket_url,
            write_transactions,
            &signers,
            config.rpc_client.commitment(),
            last_valid_slot,
        ).map_err(|err| {
            format!("Data writes to program account failed: {}", err)
        })?;
        debug!("Writing program data done");
    }

    // Create storage account if not exists
    let storage = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        debug!("Create storage account");
        let storage = create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &rng.gen::<u32>().to_string(), 128*1024 as u64)?;
        debug!("storage = {}", storage);
        storage
    };

    let accounts = vec![AccountMeta::new(holder, false),
                        AccountMeta::new(storage, false),
                        AccountMeta::new(program_id, false),
                        AccountMeta::new(program_code, false),
                        AccountMeta::new(caller_sol, false),
                        AccountMeta::new_readonly(config.evm_loader, false),
                        AccountMeta::new(clock::id(), false),
                        ];

    debug!("{}", &hex::encode(msg));
    debug!("trx_from_account_data_instruction");
    let trx_from_account_data_instruction = Instruction::new_with_bincode(config.evm_loader, &(0x0bu8, 0u64), accounts);
    send_transaction(config, &[trx_from_account_data_instruction])?;
    debug!("trx_from_account_data_instruction done");

    // Continue while no result
    loop {
        debug!("continue");
        let accounts = vec![AccountMeta::new(storage, false),
                            AccountMeta::new(program_id, false),
                            AccountMeta::new(program_code, false),
                            AccountMeta::new(caller_sol, false),
                            AccountMeta::new_readonly(config.evm_loader, false),
                            AccountMeta::new(clock::id(), false)];
        let continue_instruction = Instruction::new_with_bincode(config.evm_loader, &(0x0au8, 400u64), accounts);
        let signature = send_transaction(config, &[continue_instruction])?;
        debug!("continue done");
        let result = config.rpc_client.get_confirmed_transaction(&signature, UiTransactionEncoding::Json)?;
        debug!("got result");
        let mut return_value : Option<Vec<u8>> = None;
        if let EncodedTransaction::Json(transaction) = result.transaction.transaction {
            if let UiMessage::Raw(message) = transaction.message {
                let evm_loader_index = message.account_keys.iter().position(|x| *x == config.evm_loader.to_string());
                if let Some(meta) = result.transaction.meta {
                    if let Some(inner_instructions) = meta.inner_instructions {
                        for instruction in inner_instructions {
                            if instruction.index == 0 {
                                if let Some(UiInstruction::Compiled(compiled_instruction)) = instruction.instructions.iter().last() {
                                    if compiled_instruction.program_id_index as usize == evm_loader_index.unwrap() {
                                        let decoded = bs58::decode(compiled_instruction.data.clone()).into_vec().unwrap();
                                        if decoded[0] == 6 {
                                            debug!("success");
                                            return_value = Some(decoded[1..].to_vec());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(value) = return_value {
            let (exit_code, data) = value.split_at(1);
            debug!("exit code {}", exit_code[0]);
            debug!("return data {}", &hex::encode(data));
            break;
        }
    }

    println!("{}", json!({
        "programId": format!("{}", program_id),
        "codeId": format!("{}", program_code),
        "ethereum": format!("{:?}", program_ether),
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
                .arg(
                    Arg::with_name("caller")
                        .value_name("CALLER")
                        .takes_value(true)
                        .required(true)
                        .validator(is_valid_pubkey)
                        .help("Solana pubkey of the caller"),
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
                rpc_client: Arc::new(RpcClient::new_with_commitment(json_rpc_url, commitment)),
                websocket_url: "".to_string(),
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
                let val = arg_matches.value_of("caller").unwrap().to_string();
                let caller = Pubkey::from_str(&val).unwrap();

                command_deploy(&config, &program_location, caller)
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
