#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]

mod account_storage;
use crate::{
    account_storage::{
        make_solana_program_address,
        EmulatorAccountStorage,
        AccountJSON,
        TokenAccountJSON,
    },
};

use evm_loader::{
    instruction::EvmInstruction,
    account_data::{
        ACCOUNT_SEED_VERSION,
        AccountData,
        Account,
        Contract
    },
    config::{ token_mint, collateral_pool_base },
};

use evm::{H160, H256, U256, ExitReason,};
use solana_sdk::{
    clock::Slot,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    incinerator,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, Signature},
    signers::Signers,
    keccak::Hasher,
    transaction::Transaction,
    system_program,
    sysvar,
    system_instruction,
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
};
use serde_json::json;
use std::{
    collections::HashMap,
    io::{Read},
    fs::File,
    env,
    str::FromStr,
    process::exit,
    sync::Arc,
    thread::sleep,
    time::{Duration},
    convert::{TryFrom, TryInto},
    fmt,
    fmt::{Debug, Display,},
    cell::RefCell,
    rc::Rc
};

use clap::{
    crate_description, crate_name, value_t_or_exit, App, AppSettings, Arg,
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

use libsecp256k1::SecretKey;
use libsecp256k1::PublicKey;

use rlp::RlpStream;

use log::{debug, error, info};
use crate::account_storage::SolanaAccountJSON;
use evm_loader::{
    executor_state::{
        ExecutorState,
        ExecutorSubstate,
    },
    executor::Machine,
    solana_backend::AccountStorage,
    solidity_account::SolidityAccount
};

const DATA_CHUNK_SIZE: usize = 229; // Keep program chunks under PACKET_DATA_SIZE

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

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

#[allow(clippy::too_many_lines)]
fn command_emulate(config: &Config, contract_id: Option<H160>, caller_id: H160, data: Option<Vec<u8>>, value: Option<U256>) -> CommandResult {
    debug!("command_emulate(config={:?}, contract_id={:?}, caller_id={:?}, data={:?}, value={:?})",
        config,
        contract_id,
        caller_id,
        &hex::encode(data.clone().unwrap_or_default()),
        value);

    let storage = match &contract_id {
        Some(program_id) =>  {
            debug!("program_id to call: {:?}", *program_id);
            EmulatorAccountStorage::new(config, *program_id, caller_id)
        },
        None => {
            let (solana_address, _nonce) = make_solana_program_address(&caller_id, &config.evm_loader);
            let trx_count = get_ether_account_nonce(config, &solana_address)?;
            let trx_count= trx_count.0;
            let program_id = get_program_ether(&caller_id, trx_count);
            debug!("program_id to deploy: {:?}", program_id);
            EmulatorAccountStorage::new(config, program_id, caller_id)
        }
    };

    let (exit_reason, result, applies_logs, used_gas, steps_executed) = {
        // u64::MAX is too large, remix gives this error:
        // Gas estimation errored with the following message (see below).
        // Number can only safely store up to 53 bits
        let gas_limit = 50_000_000;
        let executor_substate = Box::new(ExecutorSubstate::new(gas_limit, &storage));
        let executor_state = ExecutorState::new(executor_substate, &storage);
        let mut executor = Machine::new(executor_state);
        debug!("Executor initialized");

        let (result, exit_reason) = match &contract_id {
            Some(_) =>  {
                debug!("call_begin(storage.origin()={:?}, storage.contract()={:?}, data={:?}, value={:?})",
                    storage.origin(),
                    storage.contract(),
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.call_begin(storage.origin(),
                                    storage.contract(),
                                    data.unwrap_or_default(),
                                    value.unwrap_or_default(),
                                    gas_limit)?;
                executor.execute()
            },
            None => {
                debug!("create_begin(storage.origin()={:?}, data={:?}, value={:?})",
                    storage.origin(),
                    &hex::encode(data.clone().unwrap_or_default()),
                    value);
                executor.create_begin(storage.origin(),
                                      data.unwrap_or_default(),
                                      value.unwrap_or_default(),
                                      gas_limit)?;
                executor.execute()
            }
        };
        debug!("Execute done, exit_reason={:?}, result={:?}", exit_reason, result);
        debug!("{} steps executed", executor.get_steps_executed());

        let steps_executed = executor.get_steps_executed();
        let executor_state = executor.into_state();
        let used_gas = executor_state.gasometer().used_gas() + 1; // "+ 1" because of https://github.com/neonlabsorg/neon-evm/issues/144
        let refunded_gas = executor_state.gasometer().refunded_gas();
        let needed_gas = used_gas + (if refunded_gas > 0 { u64::try_from(refunded_gas)? } else { 0 });
        debug!("used_gas={:?} refunded_gas={:?}", used_gas, refunded_gas);
        if exit_reason.is_succeed() {
            debug!("Succeed execution");
            let apply = executor_state.deconstruct();
            (exit_reason, result, Some(apply), needed_gas, steps_executed)
        } else {
            (exit_reason, result, None, needed_gas, steps_executed)
        }
    };

    debug!("Call done");
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, _logs, transfers, spl_transfers, spl_approves, erc20_approves) = applies_logs.unwrap();

            storage.apply(applies);
            storage.apply_transfers(transfers);
            storage.apply_spl_approves(spl_approves);
            storage.apply_spl_transfers(spl_transfers);
            storage.apply_erc20_approves(erc20_approves);

            debug!("Applies done");
            "succeed".to_string()
        }
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
        ExitReason::StepLimitReached => unreachable!(),
    };

    info!("{}", &status);
    info!("{}", &hex::encode(&result));

    if !exit_reason.is_succeed() {
        debug!("Not succeed execution");
    }

    let accounts: Vec<AccountJSON> = storage.get_used_accounts();

    let solana_accounts: Vec<SolanaAccountJSON> = storage.solana_accounts
        .borrow()
        .values()
        .cloned()
        .map(SolanaAccountJSON::from)
        .collect();

    let token_accounts: Vec<TokenAccountJSON> = storage.token_accounts
        .borrow()
        .values()
        .cloned()
        .map(TokenAccountJSON::from)
        .collect();

    let js = json!({
        "accounts": accounts,
        "solana_accounts": solana_accounts,
        "token_accounts": token_accounts,
        "result": &hex::encode(&result),
        "exit_status": status,
        "used_gas": used_gas,
        "steps_executed": steps_executed,
    }).to_string();

    println!("{}", js);

    Ok(())
}

fn command_create_program_address (
    config: &Config,
    ether_address: &H160,
) {
    let (solana_address, nonce) = make_solana_program_address(ether_address, &config.evm_loader);
    println!("{} {}", solana_address, nonce);
}

fn command_create_ether_account (
    config: &Config,
    ether_address: &H160,
    lamports: u64,
    space: u64
) -> CommandResult {
    let (solana_address, nonce) = make_solana_program_address(ether_address, &config.evm_loader);
    let token_address = spl_associated_token_account::get_associated_token_address(&solana_address, &token_mint::id());
    debug!("Create ethereum account {} <- {} {}", solana_address, hex::encode(ether_address), nonce);

    let instruction = Instruction::new_with_bincode(
            config.evm_loader,
            &EvmInstruction::CreateAccount {lamports, space, ether: *ether_address, nonce},
            vec![
                AccountMeta::new(config.signer.pubkey(), true),
                AccountMeta::new(solana_address, false),
                AccountMeta::new(token_address, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(token_mint::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(spl_associated_token_account::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
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
        "solana": solana_address.to_string(),
        "token": token_address.to_string(),
        "ether": hex::encode(ether_address),
        "nonce": nonce,
    }));

    Ok(())
}

fn read_program_data(program_location: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = File::open(program_location).map_err(|err| {
        format!("Unable to open program file '{}': {}", program_location, err)
    })?;
    let mut program_data = Vec::new();
    file.read_to_end(&mut program_data).map_err(|err| {
        format!("Unable to read program file '{}': {}", program_location, err)
    })?;

    Ok(program_data)
}

#[allow(clippy::too_many_lines)]
fn send_and_confirm_transactions_with_spinner<T: Signers>(
    rpc_client: &Arc<RpcClient>,
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
            let pending_signatures = pending_transactions.keys().copied().collect::<Vec<_>>();
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
                                    pending_transactions.remove(signature);
                                }
                            } else if status.confirmations.is_none()
                                || status.confirmations.unwrap() > 1
                            {
                                pending_transactions.remove(signature);
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
        for (_, mut transaction) in pending_transactions {
            transaction.try_sign(signer_keys, blockhash)?;
            transactions.push(transaction);
        }
    }
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

fn make_deploy_ethereum_transaction(
    trx_count: u64,
    program_data: &[u8],
    caller_private: &SecretKey,
) -> Vec<u8> {
    let rlp_data = {
        let tx = UnsignedTransaction {
            to: None,
            nonce: trx_count,
            gas_limit: 9_999_999.into(),
            gas_price: 0.into(),
            value: 0.into(),
            data: program_data.to_owned(),
            chain_id: 111.into(), // Will fixed in #61 issue
        };

        rlp::encode(&tx).to_vec()
    };

    let (sig, rec) = {
        use libsecp256k1::{Message, sign};
        let msg = Message::parse(&keccak256(rlp_data.as_slice()));
        sign(&msg, caller_private)
    };

    let mut msg : Vec<u8> = Vec::new();
    msg.extend(sig.serialize().iter().copied());
    msg.push(rec.serialize());
    msg.extend((rlp_data.len() as u64).to_le_bytes().iter().copied());
    msg.extend(rlp_data);

    msg
}

fn fill_holder_account(
    config: &Config,
    holder: &Pubkey,
    holder_id: u64,
    msg: &[u8],
) -> Result<(), Error> {
    let creator = &config.signer;
    let signers = [&*config.signer];

    // Write code to holder account
    debug!("Write code");
    let mut write_messages = vec![];
    for (chunk, i) in msg.chunks(DATA_CHUNK_SIZE).zip(0..) {
        let offset = u32::try_from(i*DATA_CHUNK_SIZE)?;

        let instruction = Instruction::new_with_bincode(
            config.evm_loader,
            /* &EvmInstruction::WriteHolder {holder_id, offset, bytes: chunk}, */
            &(0x12_u8, holder_id, offset, chunk),
            vec![AccountMeta::new(*holder, false),
                 AccountMeta::new(creator.pubkey(), true)]
        );

        let message = Message::new(&[instruction], Some(&creator.pubkey()));
        write_messages.push(message);
    }
    debug!("Send write message");

    // Send write message
    {
        let (blockhash, _, last_valid_slot) = config.rpc_client
            .get_recent_blockhash_with_commitment(CommitmentConfig::confirmed())?
            .value;

        let mut write_transactions = vec![];
        for message in write_messages {
            let mut tx = Transaction::new_unsigned(message);
            tx.try_sign(&signers, blockhash)?;
            write_transactions.push(tx);
        }

        debug!("Writing program data");
        send_and_confirm_transactions_with_spinner(
            &config.rpc_client,
            &config.websocket_url,
            write_transactions,
            &signers,
            CommitmentConfig::confirmed(),
            last_valid_slot,
        ).map_err(|err| format!("Data writes to program account failed: {}", err))?;
        debug!("Writing program data done");
    }

    Ok(())
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
    caller_sol: &Pubkey
) -> Result<(u64, H160, Pubkey), Error> {
    let data = match config.rpc_client.get_account_with_commitment(caller_sol, CommitmentConfig::confirmed())?.value{
        Some(acc) => acc.data,
        None => return Ok((u64::default(), H160::default(), Pubkey::default()))
    };

    debug!("get_ether_account_nonce data = {:?}", data);
    let account = match evm_loader::account_data::AccountData::unpack(&data) {
        Ok(acc_data) =>
            match acc_data {
            AccountData::Account(acc) => acc,
            _ => return Err("Caller has incorrect type".into())
        },
        Err(_) => return Err("Caller unpack error".into())
    };
    let trx_count = account.trx_count;
    let caller_ether = account.ether;
    let caller_token = spl_associated_token_account::get_associated_token_address(caller_sol, &token_mint::id());

    debug!("Caller: ether {}, solana {}", caller_ether, caller_sol);
    debug!("Caller trx_count: {} ", trx_count);
    debug!("caller_token = {}", caller_token);

    Ok((trx_count, caller_ether, caller_token))
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

fn get_ethereum_contract_account_credentials(
    config: &Config,
    caller_ether: &H160,
    trx_count: u64,
) -> (Pubkey, H160, u8, Pubkey, Pubkey, String) {
    let creator = &config.signer;

    let (program_id, program_ether, program_nonce) = {
        let ether = get_program_ether(caller_ether, trx_count);
        let (address, nonce) = make_solana_program_address(&ether, &config.evm_loader);
        (address, ether, nonce)
    };
    debug!("Create account: {} with {} {}", program_id, program_ether, program_nonce);

    let program_token = spl_associated_token_account::get_associated_token_address(&program_id, &token_mint::id());

    let (program_code, program_seed) = {
        let seed: &[u8] = &[ &[ACCOUNT_SEED_VERSION], program_ether.as_bytes() ].concat();
        let seed = bs58::encode(seed).into_string();
        debug!("Code account seed {} and len {}", &seed, &seed.len());
        let address = Pubkey::create_with_seed(&creator.pubkey(), &seed, &config.evm_loader).unwrap();
        (address, seed)
    };
    debug!("Create code account: {}", &program_code.to_string());

    (program_id, program_ether, program_nonce, program_token, program_code, program_seed)
}

#[allow(clippy::too_many_arguments)]
fn create_ethereum_contract_accounts_in_solana(
    config: &Config,
    program_id: &Pubkey,
    program_ether: &H160,
    program_nonce: u8,
    program_token: &Pubkey,
    program_code: &Pubkey,
    program_seed: &str,
    program_code_len: usize,
) -> Result<Vec<Instruction>, Error> {
    let account_header_size = 1+Account::SIZE;
    let contract_header_size = 1+Contract::SIZE;

    let creator = &config.signer;
    let program_code_acc_len = contract_header_size + program_code_len + 2*1024;

    let minimum_balance_for_account = config.rpc_client.get_minimum_balance_for_rent_exemption(account_header_size)?;
    let minimum_balance_for_code = config.rpc_client.get_minimum_balance_for_rent_exemption(program_code_acc_len)?;

    if let Some(_account) = config.rpc_client.get_account_with_commitment(program_id, CommitmentConfig::confirmed())?.value
    {
        return Err("Account already exist".to_string().into());
        // debug!("Account already exist");
    }

    let instructions = vec![
        system_instruction::create_account_with_seed(
            &creator.pubkey(),
            program_code,
            &creator.pubkey(),
            program_seed,
            minimum_balance_for_code,
            program_code_acc_len as u64,
            &config.evm_loader
        ),
        Instruction::new_with_bincode(
            config.evm_loader,
            &(2_u32, minimum_balance_for_account, 0_u64, program_ether.as_fixed_bytes(), program_nonce),
            vec![
                AccountMeta::new(creator.pubkey(), true),
                AccountMeta::new(*program_id, false),
                AccountMeta::new(*program_token, false),
                AccountMeta::new(*program_code, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(token_mint::id(), false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(spl_associated_token_account::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ]
        )
    ];

    Ok(instructions)
}

fn create_storage_account(config: &Config) -> Result<Pubkey, Error> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let creator = &config.signer;
    debug!("Create storage account");
    let storage = create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &rng.gen::<u32>().to_string(), 128*1024_u64)?;
    debug!("storage = {}", storage);
    Ok(storage)
}

fn get_collateral_pool_account_and_index(config: &Config) -> (Pubkey, u32) {
    let collateral_pool_index = 2;
    let seed = format!("{}{}", collateral_pool_base::PREFIX, collateral_pool_index);
    let collateral_pool_account = Pubkey::create_with_seed(
        &collateral_pool_base::id(),
        &seed,
        &config.evm_loader).unwrap();

    (collateral_pool_account, collateral_pool_index)
}

fn parse_transaction_reciept(config: &Config, result: EncodedConfirmedTransaction) -> Option<Vec<u8>> {
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
    return_value
}

fn create_account_with_seed(
    config: &Config,
    funding: &Pubkey,
    base: &Pubkey,
    seed: &str,
    len: u64
) -> Result<Pubkey, Error> {
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
) -> Result<Signature, Error> {
    let message = Message::new(instructions, Some(&config.signer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    let signers = [&*config.signer];
    let (blockhash, _, _last_valid_slot) = config.rpc_client
        .get_recent_blockhash_with_commitment(CommitmentConfig::confirmed())?
        .value;
    transaction.try_sign(&signers, blockhash)?;

    let tx_sig = config.rpc_client.send_and_confirm_transaction_with_spinner_and_config(
        &transaction,
        CommitmentConfig::confirmed(),
        RpcSendTransactionConfig {
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            ..RpcSendTransactionConfig::default()
        },
    )?;

    Ok(tx_sig)
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

#[allow(clippy::too_many_lines)]
fn command_deploy(
    config: &Config,
    program_location: &str
) -> CommandResult {
    let creator = &config.signer;
    let program_data = read_program_data(program_location)?;
    let operator_token = spl_associated_token_account::get_associated_token_address(&creator.pubkey(), &token_mint::id());

    // Create ethereum caller private key from sign of array by signer
    // let (caller_private, caller_ether, caller_sol, _caller_nonce) = get_ethereum_caller_credentials(config);

    let (caller_private_eth, caller_ether) = {
        let private_bytes : [u8; 64] = config.keypair.as_ref().unwrap().to_bytes();
        let mut sign_arr: [u8;32] = Default::default();
        sign_arr.clone_from_slice(&private_bytes[..32]);
        let caller_private = SecretKey::parse(&sign_arr).unwrap();
        let caller_public = PublicKey::from_secret_key(&caller_private);
        let caller_ether: H160 = keccak256_h256(&caller_public.serialize()[1..]).into();
        (caller_private, caller_ether)
    };

    let (caller_sol, _) = make_solana_program_address(&caller_ether, &config.evm_loader);

    if config.rpc_client.get_account_with_commitment(&caller_sol, CommitmentConfig::confirmed())?.value.is_none() {
        debug!("Caller account not found");
        command_create_ether_account(config, &caller_ether, 10_u64.pow(9), 0  )?;
    } else {
        debug!(" Caller account found");
    }

    // Get caller nonce
    let (trx_count, caller_ether, caller_token) = get_ether_account_nonce(config, &caller_sol)?;

    let (program_id, program_ether, program_nonce, program_token, program_code, program_seed) =
        get_ethereum_contract_account_credentials(config, &caller_ether, trx_count);

    // Check program account to see if partial initialization has occurred
    let mut instrstruction = create_ethereum_contract_accounts_in_solana(
        config,
        &program_id,
        &program_ether,
        program_nonce,
        &program_token,
        &program_code,
        &program_seed,
        program_data.len()
    )?;

    // Create transaction prepared for execution from account
    let msg = make_deploy_ethereum_transaction(trx_count, &program_data, &caller_private_eth);

    // Create holder account (if not exists)
    let (holder_id, holder_seed) = generate_random_holder_seed();
    let holder = create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &holder_seed, 128*1024_u64)?;

    fill_holder_account(config, &holder, holder_id, &msg)?;

    // Create storage account if not exists
    let storage = create_storage_account(config)?;

    let (collateral_pool_acc, collateral_pool_index) = get_collateral_pool_account_and_index(config);

    let accounts = vec![
                        AccountMeta::new(storage, false),

                        AccountMeta::new(creator.pubkey(), true),
                        AccountMeta::new(collateral_pool_acc, false),
                        AccountMeta::new(operator_token, false),
                        AccountMeta::new(caller_token, false),
                        AccountMeta::new(system_program::id(), false),

                        AccountMeta::new(program_id, false),
                        AccountMeta::new(program_token, false),
                        AccountMeta::new(program_code, false),
                        AccountMeta::new(caller_sol, false),
                        AccountMeta::new(caller_token, false),

                        AccountMeta::new_readonly(config.evm_loader, false),
                        AccountMeta::new_readonly(token_mint::id(), false),
                        AccountMeta::new_readonly(spl_token::id(), false),
                        ];

    let mut holder_with_accounts = vec![AccountMeta::new(holder, false)];
    holder_with_accounts.extend(accounts.clone());
    // Send trx_from_account_data_instruction
    {
        debug!("trx_from_account_data_instruction holder_plus_accounts: {:?}", holder_with_accounts);
        let trx_from_account_data_instruction = Instruction::new_with_bincode(config.evm_loader,
                                                                              &(0x16_u8, collateral_pool_index, 0_u64),
                                                                              holder_with_accounts);
        instrstruction.push(trx_from_account_data_instruction);
        send_transaction(config, &instrstruction)?;
    }

    // Continue while no result
    loop {
        let continue_accounts = accounts.clone();
        debug!("continue continue_accounts: {:?}", continue_accounts);
        let continue_instruction = Instruction::new_with_bincode(config.evm_loader,
                                                                 &(0x14_u8, collateral_pool_index, 400_u64),
                                                                 continue_accounts);
        let signature = send_transaction(config, &[continue_instruction])?;

        // Check if Continue returned some result
        let result = config.rpc_client.get_transaction_with_config(
            &signature,
            RpcTransactionConfig {
                commitment: Some(CommitmentConfig::confirmed()),
                encoding: Some(UiTransactionEncoding::Json),
            },
        )?;

        let return_value = parse_transaction_reciept(config, result);

        if let Some(value) = return_value {
            let (exit_code, data) = value.split_at(1);
            debug!("exit code {}", exit_code[0]);
            debug!("return data {}", &hex::encode(data));
            break;
        }
    }

    println!("{}", json!({
        "programId": format!("{}", program_id),
        "programToken": format!("{}", program_token),
        "codeId": format!("{}", program_code),
        "ethereum": format!("{:?}", program_ether),
    }));
    Ok(())
}

fn command_get_ether_account_data (
    config: &Config,
    ether_address: &H160,
) {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        Some((acc, balance, code_account)) => {
            let (solana_address, _solana_nonce) = make_solana_program_address(ether_address, &config.evm_loader);
            let account_data = AccountData::unpack(&acc.data).unwrap();
            let account_data = AccountData::get_account(&account_data).unwrap();

            println!("Ethereum address: 0x{}", &hex::encode(&ether_address.as_fixed_bytes()));
            println!("Solana address: {}", solana_address);

            println!("Account fields");
            println!("    ether: {}", &account_data.ether);
            println!("    nonce: {}", &account_data.nonce);
            println!("    trx_count: {}", &account_data.trx_count);
            println!("    code_account: {}", &account_data.code_account);
            println!("    ro_blocked_cnt: {}", &account_data.ro_blocked_cnt);
            println!("    rw_blocked_acc: {}",
                     if account_data.rw_blocked_acc.is_some() {
                         account_data.rw_blocked_acc.unwrap().to_string()
                     }
                     else {
                         "".to_string()
                     }
            );
            println!("    token_account: {}", &account_data.eth_token_account);
            println!("    token_amount: {}", &balance);

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
}

fn command_get_storage_at(
    config: &Config,
    ether_address: &H160,
    index: &U256
) -> CommandResult {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        Some((acc, _balance, code_account)) => {
            let account_data = AccountData::unpack(&acc.data)?;
            let mut code_data = match code_account.as_ref() {
                Some(code) => code.data.clone(),
                None => return Err(format!("Account {:#x} is not code account", ether_address).into()),
            };
            let contract_data = AccountData::unpack(&code_data)?;
            let (solana_address, _solana_nonce) = make_solana_program_address(ether_address, &config.evm_loader);
            let code_data: std::rc::Rc<std::cell::RefCell<&mut [u8]>> = Rc::new(RefCell::new(&mut code_data));
            let solidity_account = SolidityAccount::new(&solana_address, account_data,
                                                        Some((contract_data, code_data)));
            let value = solidity_account.get_storage(index);
            print!("{:#x}", value);
            Ok(())
        },
        None => {
            Err(format!("Account not found {:#x}", ether_address).into())
        }
    }
}

fn command_cancel_trx(
    config: &Config,
    storage_account: &Pubkey,
) -> CommandResult {
    let storage = config.rpc_client.get_account_with_commitment(storage_account, CommitmentConfig::processed()).unwrap().value;

    if let Some(acc) = storage {
        if acc.owner != config.evm_loader {
            return Err(format!("Invalid owner {} for storage account", acc.owner).into());
        }
        let data = AccountData::unpack(&acc.data)?;
        let data_end = data.size();
        let storage = if let AccountData::Storage(storage) = data {storage}
                else {return Err("Not storage account".to_string().into());};

        let keys: Vec<Pubkey> = {
            println!("{:?}", storage);
            let accounts_begin = data_end;
            let accounts_end = accounts_begin + storage.accounts_len * 32;
            if acc.data.len() < accounts_end {
                return Err(format!("Accounts data too small: account_data.len()={:?} < end={:?}", acc.data.len(), accounts_end).into());
            };

            acc.data[accounts_begin..accounts_end].chunks_exact(32).map(Pubkey::new).collect()
        };

        let (caller_solana, _) = make_solana_program_address(&storage.caller, &config.evm_loader);
        let (trx_count, _caller_ether, caller_token) = get_ether_account_nonce(config, &caller_solana)?;

        let operator = &config.signer.pubkey();
        let operator_token = spl_associated_token_account::get_associated_token_address(operator, &token_mint::id());

        let mut accounts_meta : Vec<AccountMeta> = vec![
            AccountMeta::new(*storage_account, false),              // Storage account
            AccountMeta::new(*operator, true),                      // Operator
            AccountMeta::new(operator_token, false),                // Operator token
            AccountMeta::new(caller_token, false),                  // Caller token
            AccountMeta::new(incinerator::id(), false),             // Incinerator
            AccountMeta::new_readonly(system_program::id(), false), // System
        ];

        let system_accounts : Vec<Pubkey> = vec![
            config.evm_loader,
            token_mint::id(),
            spl_token::id(),
            spl_associated_token_account::id(),
            sysvar::rent::id(),
            incinerator::id(),
            system_program::id(),
            sysvar::instructions::id(),
        ];

        for key in keys {
            let writable = if system_accounts.contains(&key) {false} else {
                let acc = config.rpc_client.get_account_with_commitment(&key, CommitmentConfig::processed()).unwrap().value;
                if let Some(acc) = acc {
                    if acc.owner == config.evm_loader {
                        matches!(AccountData::unpack(&acc.data)?, AccountData::Account(_))
                    } else {
                        false
                    }
                } else {false}
            };

            if writable {
                accounts_meta.push(AccountMeta::new(key, false));
            } else {
                accounts_meta.push(AccountMeta::new_readonly(key, false));
            }
        }
        for meta in &accounts_meta {
            println!("\t{:?}", meta);
        }

        let instruction = Instruction::new_with_bincode(config.evm_loader, &(21_u8, trx_count), accounts_meta);
        send_transaction(config, &[instruction])?;

    } else {
        return Err(format!("Account not found {}", &storage_account.to_string()).into());
    }
    Ok(())
}

fn read_elf_parameters(
    _config: &Config,
    program_data: &[u8],
) {
    let elf = goblin::elf::Elf::parse(program_data).expect("Unable to parse ELF file");
    elf.dynsyms.iter().for_each(|sym| {
        let name = String::from(&elf.dynstrtab[sym.st_name]);
        if name.starts_with("NEON")
        {
            let end = program_data.len();
            let from = usize::try_from(sym.st_value).unwrap_or_else(|_| panic!("Unable to cast usize from u64:{:?}", sym.st_value));
            let to = usize::try_from(sym.st_value + sym.st_size).unwrap_or_else(|err| panic!("Unable to cast usize from u64:{:?}. Error: {}", sym.st_value + sym.st_size, err));
            if to < end && from < end {
                let buf = &program_data[from..to];
                let value = std::str::from_utf8(buf).unwrap();
                println!("{}={}", name, value);
            }
            else {
                println!("{} is out of bounds", name);
            }
        }
    });
}

fn read_program_data_from_file(config: &Config,
                               program_location: &str) -> CommandResult {
    let program_data = read_program_data(program_location)?;
    let program_data = &program_data[..];
    read_elf_parameters(config, program_data);
    Ok(())
}

fn read_program_data_from_account(config: &Config) -> CommandResult {
    let account = config.rpc_client
        .get_account_with_commitment(&config.evm_loader, config.commitment)?
        .value.ok_or(format!("Unable to find the account {}", &config.evm_loader))?;

    if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
        read_elf_parameters(config, &account.data);
        Ok(())
    } else if account.owner == bpf_loader_upgradeable::id() {
        if let Ok(UpgradeableLoaderState::Program {
                      programdata_address,
                  }) = account.state()
        {
            let programdata_account = config.rpc_client
                .get_account_with_commitment(&programdata_address, config.commitment)?
                .value.ok_or(format!(
                "Failed to find associated ProgramData account {} for the program {}",
                programdata_address, &config.evm_loader))?;

            if let Ok(UpgradeableLoaderState::ProgramData { .. }) = programdata_account.state() {
                let offset =
                    UpgradeableLoaderState::programdata_data_offset().unwrap_or(0);
                let program_data = &programdata_account.data[offset..];
                read_elf_parameters(config, program_data);
                Ok(())
            } else {
                Err(
                    format!("Invalid associated ProgramData account {} found for the program {}",
                            programdata_address, &config.evm_loader)
                        .into(),
                )
            }

        } else if let Ok(UpgradeableLoaderState::Buffer { .. }) = account.state() {
            let offset = UpgradeableLoaderState::buffer_data_offset().unwrap_or(0);
            let program_data = &account.data[offset..];
            read_elf_parameters(config, program_data);
            Ok(())
        } else {
            Err(format!(
                "{} is not an upgradeble loader buffer or program account",
                &config.evm_loader
            )
                .into())
        }
    } else {
        Err(format!("{} is not a BPF program", &config.evm_loader).into())
    }
}

fn command_neon_elf(
    config: &Config,
    program_location: Option<&str>,
) -> CommandResult {
    program_location.map_or_else(
        || read_program_data_from_account(config),
        |program_location| read_program_data_from_file(config, program_location),
    )
}

fn command_update_valids_table(
    config: &Config,
    ether_address: &H160,
) -> CommandResult {
    let account_data = if let Some((account, _, _)) = EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        AccountData::unpack(&account.data)?
    } else {
        return Err(format!("Account not found {:#x}", ether_address).into());
    };

    let code_account = account_data.get_account()?.code_account;
    if code_account == Pubkey::new_from_array([0_u8; 32]) {
        return Err(format!("Code account not found {:#x}", ether_address).into());
    }

    let instruction = Instruction::new_with_bincode(
        config.evm_loader,
        &(23),
        vec![AccountMeta::new(code_account, false)]
    );

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

    Ok(())
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

fn is_amount_u256<T>(amount: T) -> Result<(), String>
    where
        T: AsRef<str> + Display,
{
    if amount.as_ref().parse::<U256>().is_ok() {
        Ok(())
    } else {
        Err(format!(
            "Unable to parse input amount as integer U256, provided: {}",
            amount
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
                        .validator(is_amount_u256)
                        .help("Transaction value")
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

        let verbosity = usize::try_from(app_matches.occurrences_of("verbose")).unwrap_or_else(|_| {
            error!("Invalid message verbosity");
            exit(1);
        });
        stderrlog::new()
            .module(module_path!())
            .verbosity(verbosity)
            .init()
            .unwrap();

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

            let evm_loader = pubkey_of(&app_matches, "evm_loader")
                    .unwrap_or_else(|| {
                        error!("Need specify evm_loader");
                        exit(1);
                    });

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
                    exit(1);
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
                // fee_payer,
                signer,
                keypair,
                commitment,
            }
        };

        let (sub_command, sub_matches) = app_matches.subcommand();
        let result = match (sub_command, sub_matches) {
            ("emulate", Some(arg_matches)) => {
                let contract = h160_or_deploy_of(arg_matches, "contract");
                let sender = h160_of(arg_matches, "sender").unwrap();
                let data = hexdata_of(arg_matches, "data");
                let value = value_of(arg_matches, "value");

                command_emulate(&config, contract, sender, data, value)
            }
            ("create-program-address", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "seed").unwrap();

                command_create_program_address(&config, &ether);

                Ok(())
            }
            ("create-ether-account", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "ether").unwrap();
                let lamports = value_t_or_exit!(arg_matches, "lamports", u64);
                let space = value_t_or_exit!(arg_matches, "space", u64);

                command_create_ether_account(&config, &ether, lamports, space)
            }
            ("deploy", Some(arg_matches)) => {
                let program_location = arg_matches.value_of("program_location").unwrap().to_string();

                command_deploy(&config, &program_location)
            }
            ("get-ether-account-data", Some(arg_matches)) => {
                let ether = h160_of(arg_matches, "ether").unwrap();

                command_get_ether_account_data(&config, &ether);

                Ok(())
            }
            ("cancel-trx", Some(arg_matches)) => {
                let storage_account = pubkey_of(arg_matches, "storage_account").unwrap();

                command_cancel_trx(&config, &storage_account)
            }
            ("neon-elf-params", Some(arg_matches)) => {
                let program_location = arg_matches.value_of("program_location");

                command_neon_elf(&config, program_location)
            }
            ("get-storage-at", Some(arg_matches)) => {
                let contract_id = h160_of(arg_matches, "contract_id").unwrap();
                let index = u256_of(arg_matches, "index").unwrap();

                command_get_storage_at(&config, &contract_id, &index)
            }
            ("update-valids-table", Some(arg_matches)) => {
                let contract_id = h160_of(arg_matches, "contract_id").unwrap();

                command_update_valids_table(&config, &contract_id)
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
