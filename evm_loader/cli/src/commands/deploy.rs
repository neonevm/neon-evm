use std::{
    collections::HashMap,
    sync::Arc,
    thread::sleep,
    time::{Duration},
    convert::{TryFrom},
};
use log::{debug};

use libsecp256k1::SecretKey;
use libsecp256k1::PublicKey;

use solana_sdk::{
    clock::Slot,
    commitment_config::{CommitmentConfig},
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    transaction::Transaction,
    signers::Signers,
    system_program,
    sysvar,
    system_instruction,
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

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcTransactionConfig},
    rpc_config::{RpcSendTransactionConfig},
    rpc_request::MAX_GET_SIGNATURE_STATUSES_QUERY_ITEMS,
    tpu_client::{TpuClient, TpuClientConfig},
};

use evm::{H160};

use evm_loader::{
    account_data::{
        Account,
        Contract
    },
};

use crate::{
    errors::NeonCliError,
    Config,
    NeonCliResult,
};


const DATA_CHUNK_SIZE: usize = 229; // Keep program chunks under PACKET_DATA_SIZE

fn get_ethereum_contract_account_credentials(
    config: &Config,
    caller_ether: &H160,
    trx_count: u64,
    token_mint: &Pubkey
) -> (Pubkey, H160, u8, Pubkey, Pubkey, String) {
    let creator = &config.signer;

    let (program_id, program_ether, program_nonce) = {
        let ether = crate::get_program_ether(caller_ether, trx_count);
        let (address, nonce) = crate::make_solana_program_address(&ether, &config.evm_loader);
        (address, ether, nonce)
    };
    debug!("Create account: {} with {} {}", program_id, program_ether, program_nonce);

    let program_token = spl_associated_token_account::get_associated_token_address(&program_id, token_mint);

    let (program_code, program_seed) = {
        let seed: &[u8] = &[ &[crate::ACCOUNT_SEED_VERSION], program_ether.as_bytes() ].concat();
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
    token_mint: &Pubkey
) -> Result<Vec<Instruction>, NeonCliError> {
    let account_header_size = 1+Account::SIZE;
    let contract_header_size = 1+Contract::SIZE;

    let creator = &config.signer;
    let program_code_acc_len = contract_header_size + program_code_len + 2*1024;

    let minimum_balance_for_account = config.rpc_client.get_minimum_balance_for_rent_exemption(account_header_size)?;
    let minimum_balance_for_code = config.rpc_client.get_minimum_balance_for_rent_exemption(program_code_acc_len)?;

    if let Some(account) = config.rpc_client.get_account_with_commitment(program_id, CommitmentConfig::confirmed())?.value
    {
        return Err(NeonCliError::AccountAlreadyExists(account));
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
                AccountMeta::new_readonly(*token_mint, false),
                AccountMeta::new_readonly(spl_token::id(), false),
                AccountMeta::new_readonly(spl_associated_token_account::id(), false),
                AccountMeta::new_readonly(sysvar::rent::id(), false),
            ]
        )
    ];

    Ok(instructions)
}

fn fill_holder_account(
    config: &Config,
    holder: &Pubkey,
    holder_id: u64,
    msg: &[u8],
) -> Result<(), NeonCliError> {
    let creator = &config.signer;
    let signers = [&*config.signer];

    // Write code to holder account
    debug!("Write code");
    let mut write_messages = vec![];
    for (chunk, i) in msg.chunks(DATA_CHUNK_SIZE).zip(0..) {
        let offset = u32::try_from(i*DATA_CHUNK_SIZE).unwrap();

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
        )?;
        debug!("Writing program data done");
    }

    Ok(())
}

fn make_deploy_ethereum_transaction(
    trx_count: u64,
    program_data: &[u8],
    caller_private: &SecretKey,
    chain_id: u64
) -> Vec<u8> {
    let rlp_data = {
        let tx = crate::UnsignedTransaction {
            to: None,
            nonce: trx_count,
            gas_limit: 999_999_999_999_u64.into(),
            gas_price: 0.into(),
            value: 0.into(),
            data: program_data.to_owned(),
            chain_id: chain_id.into(),
        };

        rlp::encode(&tx).to_vec()
    };

    let (sig, rec) = {
        use libsecp256k1::{Message, sign};
        let msg = Message::parse(&crate::keccak256(rlp_data.as_slice()));
        sign(&msg, caller_private)
    };

    let mut msg : Vec<u8> = Vec::new();
    msg.extend(sig.serialize().iter().copied());
    msg.push(rec.serialize());
    msg.extend((rlp_data.len() as u64).to_le_bytes().iter().copied());
    msg.extend(rlp_data);

    msg
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

#[allow(clippy::too_many_lines)]
fn send_and_confirm_transactions_with_spinner<T: Signers>(
    rpc_client: &Arc<RpcClient>,
    websocket_url: &str,
    mut transactions: Vec<Transaction>,
    signer_keys: &T,
    commitment: CommitmentConfig,
    mut last_valid_slot: Slot,
) -> NeonCliResult {
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
            return Err(NeonCliError::TransactionFailed);
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

#[allow(clippy::too_many_lines)]
pub fn execute(
    config: &Config,
    program_location: &str,
    token_mint: &Pubkey,
    collateral_pool_base: &Pubkey,
    chain_id: u64
) -> NeonCliResult {
    let creator = &config.signer;
    let program_data = crate::read_program_data(program_location)?;
    let operator_token = spl_associated_token_account::get_associated_token_address(&creator.pubkey(), token_mint);

    // Create ethereum caller private key from sign of array by signer
    // let (caller_private, caller_ether, caller_sol, _caller_nonce) = get_ethereum_caller_credentials(config);

    let (caller_private_eth, caller_ether) = {
        let private_bytes : [u8; 64] = config.keypair.as_ref().unwrap().to_bytes();
        let mut sign_arr: [u8;32] = Default::default();
        sign_arr.clone_from_slice(&private_bytes[..32]);
        let caller_private = SecretKey::parse(&sign_arr).unwrap();
        let caller_public = PublicKey::from_secret_key(&caller_private);
        let caller_ether: H160 = crate::keccak256_h256(&caller_public.serialize()[1..]).into();
        (caller_private, caller_ether)
    };

    let (caller_sol, _) = crate::make_solana_program_address(&caller_ether, &config.evm_loader);

    if config.rpc_client.get_account_with_commitment(&caller_sol, CommitmentConfig::confirmed())?.value.is_none() {
        debug!("Caller account not found");
        crate::commands::create_ether_account::execute(config, &caller_ether, 10_u64.pow(9), 0, token_mint)?;
    } else {
        debug!(" Caller account found");
    }

    // Get caller nonce
    let (trx_count, caller_ether, caller_token) = crate::get_ether_account_nonce(config, &caller_sol, token_mint)?;

    let (program_id, program_ether, program_nonce, program_token, program_code, program_seed) =
        get_ethereum_contract_account_credentials(config, &caller_ether, trx_count, token_mint);

    // Check program account to see if partial initialization has occurred
    let mut instrstruction = create_ethereum_contract_accounts_in_solana(
        config,
        &program_id,
        &program_ether,
        program_nonce,
        &program_token,
        &program_code,
        &program_seed,
        program_data.len(),
        token_mint
    )?;

    // Create transaction prepared for execution from account
    let msg = make_deploy_ethereum_transaction(trx_count, &program_data, &caller_private_eth, chain_id);

    // Create holder account (if not exists)
    let (holder_id, holder_seed) = crate::generate_random_holder_seed();
    let holder = crate::create_account_with_seed(config, &creator.pubkey(), &creator.pubkey(), &holder_seed, 128*1024_u64)?;

    fill_holder_account(config, &holder, holder_id, &msg)?;

    // Create storage account if not exists
    let storage = crate::create_storage_account(config)?;

    let (collateral_pool_acc, collateral_pool_index) = crate::get_collateral_pool_account_and_index(config, collateral_pool_base);

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
                        AccountMeta::new_readonly(*token_mint, false),
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
        crate::send_transaction(config, &instrstruction)?;
    }

    // Continue while no result
    loop {
        let continue_accounts = accounts.clone();
        debug!("continue continue_accounts: {:?}", continue_accounts);
        let continue_instruction = Instruction::new_with_bincode(config.evm_loader,
                                                                 &(0x14_u8, collateral_pool_index, 400_u64),
                                                                 continue_accounts);
        let signature = crate::send_transaction(config, &[continue_instruction])?;

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

    println!("{}", serde_json::json!({
        "programId": format!("{}", program_id),
        "programToken": format!("{}", program_token),
        "codeId": format!("{}", program_code),
        "ethereum": format!("{:?}", program_ether),
    }));
    Ok(())
}

