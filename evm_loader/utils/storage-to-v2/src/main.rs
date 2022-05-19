#![allow(deprecated)]

use std::cmp::min;
use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;

use anyhow::Result;
use evm_core::U256;
use evm_loader::account::{AccountData, ether_account, ether_contract, ether_storage, EthereumAccount, EthereumStorage, Packable};
use evm_loader::account_storage::{AccountStorage, ProgramAccountStorage};
use evm_loader::config::{chain_id, STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT};
use jsonrpc::{Request, serde_json};
use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::{RpcClient, serialize_and_encode};
use solana_program::account_info::AccountInfo;
use solana_program::hash::Hash;
use solana_sdk::account::{Account, ReadableAccount};
use solana_sdk::account_info::IntoAccountInfo;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::{Keypair, read_keypair_file};
use solana_sdk::signer::Signer;
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;
use solana_transaction_status::UiTransactionEncoding;

#[derive(serde_derive::Deserialize)]
struct Config {
    url: String,
    evm_loader_pubkey: String,
    batch_size: usize,
}

const PAYER_KEYPAIR_PATH: &str = "keys.json";

type AccountsMap = HashMap<Pubkey, Account>;

lazy_static::lazy_static! {
    static ref CONFIG: Config = serde_json::from_reader(File::open("config.json").unwrap()).unwrap();
    static ref EVM_LOADER: Pubkey = Pubkey::from_str(&CONFIG.evm_loader_pubkey).unwrap();
    static ref PAYER: Keypair = read_keypair_file(PAYER_KEYPAIR_PATH).unwrap();
}

fn write_value_instruction(
    ether_account: Pubkey,
    key: U256,
    value: U256,
) -> Instruction {
    let mut data = vec![28_u8; 1 + 32 + 32];
    key.to_big_endian(&mut data[1..33]);
    value.to_big_endian(&mut data[33..]);

    Instruction::new_with_bincode(
        EVM_LOADER.clone(),
        &data,
        vec![
            AccountMeta::new_readonly(PAYER.pubkey(), true),         // Operator
            AccountMeta::new_readonly(system_program::id(), false),  // System program
            AccountMeta::new_readonly(ether_account, false),         // Ether account
        ],
    )
}

fn convert_to_v2_instruction(
    ether_account: Pubkey,
) -> Instruction {
    Instruction::new_with_bincode(
        EVM_LOADER.clone(),
        &[29u8],
        vec![
            AccountMeta::new(ether_account, false),                  // Ether account
        ],
    )
}

fn get_evm_accounts<T: FromIterator<(Pubkey, Account)>>(
    client: &RpcClient,
    tags_sorted: &[u8],
) -> ClientResult<T> {
    Ok(
        client.get_program_accounts(&EVM_LOADER)?
            .into_iter()
            .filter(|(_pubkey, account)|
                account.data.len() > 0 && tags_sorted.binary_search(&account.data[0]).is_ok()
            )
            .collect()
    )
}

fn copy_data_to_distributed_storage<'a>(
    ethereum_contract_v1: AccountData<'a, ether_contract::DataV1, ether_contract::ExtensionV1<'a>>,
    recent_blockhash: &Hash,
) -> Vec<Transaction> {
    let storage_entries_in_contract_account = U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT);
    let mut result = Vec::new();
    for (key, value) in ethereum_contract_v1.extension.storage.iter() {
        if key < storage_entries_in_contract_account {
            continue;
        }
        
        let instructions = vec![
            write_value_instruction(ethereum_contract_v1.info.key.clone(), key, value),
        ];
        let mut message = Message::new(&instructions, Some(&PAYER.pubkey()));
        message.recent_blockhash = recent_blockhash.clone();
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&*PAYER], recent_blockhash.clone());
        result.push(transaction);
    }

    result
}

fn send_batch_slice(client: &jsonrpc::client::Client, batch_slice: &[Transaction]) -> Result<()> {
    let mut requests_params = Vec::with_capacity(batch_slice.len());
    for transaction in batch_slice {
        let serialized = serialize_and_encode(transaction, UiTransactionEncoding::Base64)?;
        requests_params.push([
            jsonrpc::try_arg(serialized)?,
            jsonrpc::try_arg(serde_json::json!({ "encoding": "base64" }))?,
        ]);
    }

    let requests: Vec<Request> = requests_params.iter()
        .map(|params| client.build_request("sendTransaction", params))
        .collect();

    let responses = client.send_batch(&requests)?;
    let error_count = responses.iter()
        .filter_map(|response_opt| response_opt.as_ref()
            .map(|response| response.error.as_ref()).flatten()
        )
        .map(|error| println!("Error: {}", error.message))
        .count();
    if error_count == 0 {
        println!("OK")
    } else {
        println!("Error count: {}", error_count);
    }

    Ok(())
}

fn send_batch(client: &jsonrpc::client::Client, batch: &[Transaction]) -> Result<()> {
    let mut from = 0;
    while from < batch.len() {
        let to = from + CONFIG.batch_size;
        println!("Sending batch ({}..{} of {} requests)...", from, to, batch.len());
        send_batch_slice(client, &batch[from..min(to, batch.len())])?;
        from += CONFIG.batch_size;
    }

    Ok(())
}

fn is_data_written<'a>(
    storage: &ProgramAccountStorage<'a>,
    accounts_map: &mut AccountsMap,
    ethereum_contract_v1: &AccountData<'a, ether_contract::DataV1, ether_contract::ExtensionV1<'a>>,
) -> bool {
    let mut ether_account_backend = accounts_map
        .get(&ethereum_contract_v1.owner)
        .expect(
            &format!(
                "Failed to find Ethereum account (Solana account {}) for code contract {}",
                ethereum_contract_v1.owner,
                ethereum_contract_v1.info.key,
            )
        )
        .clone();
    let ether_account_info = (&ethereum_contract_v1.owner, &mut ether_account_backend).into_account_info();
    let ether_account = EthereumAccount::from_account(&EVM_LOADER, &ether_account_info)
        .expect(
            &format!(
                "Failed to create Ethereum account from data of account: {}",
                ether_account_info.key,
            )
        );

    let ether_address = ether_account.address;
    let storage_entries_in_contract_account = U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT);
    for (key, value) in ethereum_contract_v1.extension.storage.iter() {
        if key < storage_entries_in_contract_account {
            continue;
        }
        let (solana_address, _) = storage.get_storage_address(&ether_address, &key);
        let mut account = match accounts_map.get(&solana_address) {
            Some(account) => account.clone(),
            None  => return false,
        };

        let info = (&solana_address, &mut account).into_account_info();

        if *info.owner != *EVM_LOADER {
            panic!(
                "Owner of storage account is incorrect. Expected {}, but actual is {}",
                *EVM_LOADER,
                info.owner,
            );
        }

        let account_storage = EthereumStorage::from_account(&EVM_LOADER, &info)
            .expect(
                &format!(
                    "Failed to construct storage account from data of account: {}",
                    info.key,
                )
            );

        if account_storage.value != value {
            panic!(
                "Value of a storage account {} is incorrect. Expected {}, but actual is {}",
                solana_address,
                value,
                account_storage.value,
            );
        }
    }

    true
}

fn extract_data_to_distributed_storage(
    client: &RpcClient,
    json_rpc_client: &jsonrpc::client::Client,
) -> Result<usize> {
    let recent_blockhash = client.get_latest_blockhash()?;
    let mut batch = Vec::with_capacity(CONFIG.batch_size);
    let contract_accounts: Vec<(Pubkey, Account)> = get_evm_accounts(&client, &[ether_contract::DataV1::TAG])?;
    for (pubkey, mut account) in contract_accounts {
        let info = (&pubkey, &mut account).into_account_info();
        let ethereum_contract_v1 =
            AccountData::<ether_contract::DataV1, ether_contract::ExtensionV1>::from_account(
                &EVM_LOADER,
                &info,
            )?;

        let mut transactions =
            copy_data_to_distributed_storage(ethereum_contract_v1, &recent_blockhash);
        batch.append(&mut transactions);
    }

    send_batch(&json_rpc_client, &batch)?;

    Ok(batch.len())
}

fn make_convert_to_v2_transaction(pubkey: Pubkey, recent_blockhash: Hash) -> Transaction {
    let instructions = vec![
        convert_to_v2_instruction(pubkey),
    ];
    let mut message = Message::new(&instructions, Some(&PAYER.pubkey()));
    message.recent_blockhash = recent_blockhash.clone();
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[&*PAYER], recent_blockhash);

    transaction
}

fn convert_accounts_to_v2(
    client: &RpcClient,
    json_rpc_client: &jsonrpc::client::Client,
) -> Result<usize> {
    let recent_blockhash = client.get_latest_blockhash()?;

    let account_storage = ProgramAccountStorage::new(
        &EVM_LOADER,
        &[],
        evm_loader::config::token_mint::id(),
        chain_id().as_u64(),
    )?;

    let mut batch = Vec::new();
    let mut accounts_map: AccountsMap = get_evm_accounts(
        client,
        &[ether_account::DataV1::TAG, ether_contract::DataV1::TAG, ether_storage::Data::TAG],
    )?;
    let mut contracts_v1: Vec<(Pubkey, Account)> = accounts_map.iter()
        .filter_map(|(pubkey, account)|
            if account.data()[0] == ether_contract::DataV1::TAG {
                Some((pubkey.clone(), account.clone()))
            } else {
                None
            }
        )
        .collect();
    let accounts_info: Vec<AccountInfo> = contracts_v1.iter_mut()
        .map(|(pubkey, account)| (&*pubkey, account).into_account_info())
        .collect();
    for info in accounts_info.iter() {
        let ethereum_contract_v1 =
            AccountData::<ether_contract::DataV1, ether_contract::ExtensionV1>::from_account(
                &EVM_LOADER,
                &info,
            )?;
        if is_data_written(&account_storage, &mut accounts_map, &ethereum_contract_v1) {
            batch.push(make_convert_to_v2_transaction(info.key.clone(), recent_blockhash.clone()));
        }
    }

    send_batch(&json_rpc_client, &batch)?;

    Ok(batch.len())
}

fn main() -> Result<()> {
    let client = RpcClient::new(&CONFIG.url);
    let json_rpc_client = jsonrpc::client::Client::with_transport(
        jsonrpc::simple_http::SimpleHttpTransport::builder().url(&CONFIG.url)?.build()
    );

    loop {
        let extract_count = extract_data_to_distributed_storage(&client, &json_rpc_client)?;
        let convert_count = convert_accounts_to_v2(&client, &json_rpc_client)?;

        if extract_count + convert_count == 0 {
            return Ok(());
        }
    }
}
