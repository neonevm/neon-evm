#![allow(deprecated)]

use std::cmp::min;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::mem::size_of;
use std::ops::Sub;
use std::str::FromStr;
use std::time::{Duration, Instant};

use anyhow::Result;
use evm_core::{H160, U256};
use evm_loader::account::{ACCOUNT_SEED_VERSION, AccountData, ether_account, ether_contract, ether_storage, Packable};
use evm_loader::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use serde_json::{json, Value};
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::{RpcClient, serialize_and_encode};
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter;
use solana_client::rpc_filter::{MemcmpEncodedBytes, RpcFilterType};
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

use crate::json_rpc::JsonRpcClient;

mod json_rpc;

macro_rules! print {
    ($($arg:tt)*) => {
        std::print!($($arg)*);
        std::io::stdout().flush().unwrap();
    }
}

macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ({
        std::println!($($arg)*);
        std::io::stdout().flush().unwrap();
    })
}

#[derive(serde_derive::Deserialize)]
struct Config {
    url: String,
    evm_loader_pubkey: String,
    batch_size: usize,
    recent_block_hash_ttl_sec: u64,
    client_timeout_sec: u64,
}

type EthereumContractV1<'a> = AccountData<'a, ether_contract::DataV1, ether_contract::ExtensionV1<'a>>;
type ContractsV1Map<'a> = HashMap<&'a Pubkey, EthereumContractV1<'a>>;
type EtherAddressesMap = HashMap<Pubkey, H160>;
type DataWrittenMap = HashMap<Pubkey, U256>;

lazy_static::lazy_static! {
    static ref CONFIG: Config = serde_json::from_reader(
        File::open("config.json").expect("Failed to open `config.json` file"),
    ).expect("Failed to parse configuration file");
    static ref EVM_LOADER: Pubkey = Pubkey::from_str(&CONFIG.evm_loader_pubkey)
        .expect("Failed to parse `evm_loader_pubkey` in config");
    static ref PAYER: Keypair = read_keypair_file("payer.keys.json")
        .expect("Failed to read `payer.keys.json` file");
}

struct RecentBlockHash<'a> {
    client: &'a RpcClient,
    hash: Hash,
    time: Instant,
}

impl <'a> RecentBlockHash<'a> {
    fn new(client: &'a RpcClient) -> Self {
        Self {
            client,
            hash: Hash::new_from_array([0; 32]),
            time: Instant::now().sub(Duration::from_secs(60 * 60 * 24)),
        }
    }

    fn get(&mut self) -> ClientResult<&Hash> {
        if Instant::now().duration_since(self.time).as_secs() > CONFIG.recent_block_hash_ttl_sec {
            self.hash = self.client.get_latest_blockhash()?;
            self.time = Instant::now();
            println!("New recent block hash: {}", self.hash);
        }

        Ok(&self.hash)
    }
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
            AccountMeta::new_readonly(PAYER.pubkey(), true),         // Funding account
            AccountMeta::new_readonly(system_program::id(), false),  // System program
            AccountMeta::new(ether_account, false),                  // Ether account
        ],
    )
}

fn get_storage_address(address: &H160, index: &U256) -> Pubkey {
    let mut index_bytes = [0_u8; 32];
    index.to_little_endian(&mut index_bytes);

    let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ContractStorage", address.as_bytes(), &[0; size_of::<u32>()], &index_bytes];

    Pubkey::find_program_address(seeds, &EVM_LOADER).0
}

fn get_evm_accounts(
    client: &RpcClient,
    tag: u8,
    data_slice: Option<UiDataSliceConfig>,
) -> ClientResult<Vec<(Pubkey, Account)>> {
    client.get_program_accounts_with_config(
        &EVM_LOADER,
        RpcProgramAccountsConfig {
            filters: Some(
                vec![
                    RpcFilterType::Memcmp(
                        rpc_filter::Memcmp {
                            offset: 0,
                            bytes: MemcmpEncodedBytes::Bytes(vec![tag]),
                            encoding: None,
                        }
                    ),
                ]
            ),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                data_slice,
                ..Default::default()
            },
            ..Default::default()
        },
    )
}

fn copy_data_to_distributed_storage<'a>(
    ethereum_contract_v1: &EthereumContractV1<'a>,
    ether_addresses_map: &EtherAddressesMap,
    data_written_map: &DataWrittenMap,
    recent_blockhash: &Hash,
) -> Vec<Transaction> {
    let ether_address = ether_addresses_map.get(&ethereum_contract_v1.owner)
        .expect(&format!("Ethereum address not found for Solana account: {}", ethereum_contract_v1.owner));
    let storage_entries_in_contract_account = U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT);
    let mut result = Vec::new();
    for (key, value) in ethereum_contract_v1.extension.storage.iter() {
        if key < storage_entries_in_contract_account {
            continue;
        }

        let storage_address = get_storage_address(ether_address, &key);
        if let Some(stored_value) = data_written_map.get(&storage_address) {
            if stored_value == &value {
                continue;
            }
            unreachable!("Something went wrong! {} != {}", value, stored_value);
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

fn send_batch_slice(client: &JsonRpcClient, batch_slice: &[Transaction]) -> Result<()> {
    let mut requests = Vec::with_capacity(batch_slice.len());
    for transaction in batch_slice {
        let serialized = serialize_and_encode(transaction, UiTransactionEncoding::Base64)?;
        requests.push(client.request(
            "sendTransaction",
            json!([
                serialized,
                { "encoding": "base64" },
            ])
        ));
    }

    let responses = client.send_batch(&requests)?;

    if let Value::Array(responses) = responses {
        let error_count = responses.into_iter()
            .filter(|response|
                if let Value::String(ref error_message) = response["error"]["message"] {
                    println!("Error: {}", error_message);
                    true
                } else {
                    false
                }
            ).count();
        if error_count == 0 {
            println!("OK")
        } else {
            println!("Error count: {}", error_count);
        }
    } else {
        println!("Error: {:?}", responses);
    }

    Ok(())
}

fn send_batch(client: &JsonRpcClient, batch: &[Transaction]) -> Result<()> {
    let mut from = 0;
    while from < batch.len() {
        let mut to = min(from + CONFIG.batch_size, batch.len());
        if to + CONFIG.batch_size / 3 >= batch.len() {
            to = batch.len();
        }
        println!("Sending batch ({}..{} of {} requests)...", from, to, batch.len());
        send_batch_slice(client, &batch[from..to])?;
        from = to;
    }

    Ok(())
}

fn is_data_written<'a>(
    ether_addresses_map: &EtherAddressesMap,
    data_written_map: &DataWrittenMap,
    ethereum_contract_v1: &EthereumContractV1,
) -> bool {
    let ether_address = ether_addresses_map.get(&ethereum_contract_v1.owner)
        .expect(
            &format!(
                "Unable to find Ethereum address for solana account: {}",
                ethereum_contract_v1.owner,
            )
        );
    let storage_entries_in_contract_account = U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT);
    for (key, value) in ethereum_contract_v1.extension.storage.iter() {
        if key < storage_entries_in_contract_account {
            continue;
        }
        let solana_address = get_storage_address(&ether_address, &key);
        let stored_value = match data_written_map.get(&solana_address) {
            Some(value) => value,
            None => return false,
        };

        if stored_value != &value {
            panic!(
                "Value of a storage account {} is incorrect. Expected {}, but actual is {}",
                solana_address,
                value,
                stored_value,
            );
        }
    }

    true
}

fn extract_data_to_distributed_storage(
    json_rpc_client: &JsonRpcClient,
    recent_block_hash: &mut RecentBlockHash,
    ether_addresses_map: &EtherAddressesMap,
    contracts_v1_map: &ContractsV1Map,
    data_written_map: &DataWrittenMap,
) -> Result<()> {
    let mut batch = Vec::with_capacity(CONFIG.batch_size);
    for ethereum_contract_v1 in contracts_v1_map.values() {
        let mut transactions = copy_data_to_distributed_storage(
            ethereum_contract_v1,
            ether_addresses_map,
            data_written_map,
            recent_block_hash.get()?,
        );
        batch.append(&mut transactions);

        if batch.len() >= CONFIG.batch_size {
            send_batch(&json_rpc_client, &batch)?;
            batch.clear();
        }
    }

    send_batch(&json_rpc_client, &batch)
}

fn make_convert_to_v2_transaction(pubkey: Pubkey, recent_blockhash: &Hash) -> Transaction {
    let instructions = vec![
        convert_to_v2_instruction(pubkey),
    ];
    let mut message = Message::new(&instructions, Some(&PAYER.pubkey()));
    message.recent_blockhash = recent_blockhash.clone();
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[&*PAYER], recent_blockhash.clone());

    transaction
}

fn convert_accounts_to_v2(
    json_rpc_client: &JsonRpcClient,
    recent_block_hash: &mut RecentBlockHash,
    ether_addresses_map: &EtherAddressesMap,
    contracts_v1_map: &ContractsV1Map,
    data_written_map: &DataWrittenMap,
) -> Result<()> {
    let mut batch = Vec::new();

    for ethereum_contract_v1 in contracts_v1_map.values() {
        if is_data_written(ether_addresses_map, data_written_map, &ethereum_contract_v1) {
            batch.push(
                make_convert_to_v2_transaction(
                    ethereum_contract_v1.info.key.clone(),
                    recent_block_hash.get()?,
                ),
            );
            if batch.len() >= CONFIG.batch_size {
                send_batch(&json_rpc_client, &batch)?;
                batch.clear();
            }
        }
    }

    send_batch(&json_rpc_client, &batch)
}

fn obtain_ether_addresses_map(client: &RpcClient) -> ClientResult<EtherAddressesMap> {
    get_evm_accounts(
        client,
        ether_account::Data::TAG,
        Some(UiDataSliceConfig { offset: 1, length: size_of::<H160>() }),
    ).map(|vec| vec.into_iter()
        .map(|(pubkey, account)| {
            (pubkey, H160::from_slice(account.data()))
        })
        .collect()
    )
}

fn obtain_data_written_map(client: &RpcClient) -> ClientResult<DataWrittenMap> {
    get_evm_accounts(
        &client,
        ether_storage::Data::TAG,
        Some(UiDataSliceConfig { offset: 1, length: size_of::<U256>() }),
    ).map(|vec| vec.into_iter()
        .map(|(pubkey, account)|
            (pubkey, U256::from_big_endian_fast(&account.data[..]))
        )
        .collect()
    )
}

fn main() -> Result<()> {
    println!("Payer public key: {}", PAYER.pubkey());

    let client = RpcClient::new_with_timeout(
        &CONFIG.url,
        Duration::from_secs(CONFIG.client_timeout_sec),
    );
    let json_rpc_client = JsonRpcClient::new(&CONFIG.url);

    print!("Querying accounts for Ethereum addresses map... ");
    let ether_addresses_map = obtain_ether_addresses_map(&client)?;
    println!("OK ({} accounts)", ether_addresses_map.len());

    print!("Querying Contract V1 accounts... ");
    let mut contract_v1_accounts = get_evm_accounts(&client, ether_contract::DataV1::TAG, None)?;
    print!("Transforming... ");
    let contracts_v1_info: Vec<AccountInfo> = contract_v1_accounts.iter_mut()
        .map(|(pubkey, account)| (&*pubkey, account).into_account_info())
        .collect();
    let mut contracts_v1_map: ContractsV1Map = contracts_v1_info.iter()
        .map(|info| (
            info.key,
            EthereumContractV1::from_account(&EVM_LOADER, info)
                .expect(&format!("Cannot decode contract V1 data for account: {}", info.key)),
        ))
        .collect();
    println!("OK ({} accounts)", contracts_v1_info.len());

    let mut recent_block_hash = RecentBlockHash::new(&client);
    loop {
        print!("Querying infinite storage accounts... ");
        let data_written_map = obtain_data_written_map(&client)?;
        println!("OK ({} values)", data_written_map.len());

        extract_data_to_distributed_storage(
            &json_rpc_client,
            &mut recent_block_hash,
            &ether_addresses_map,
            &contracts_v1_map,
            &data_written_map,
        )?;

        convert_accounts_to_v2(
            &json_rpc_client,
            &mut recent_block_hash,
            &ether_addresses_map,
            &contracts_v1_map,
            &data_written_map,
        )?;

        let contracts_v2 = get_evm_accounts(
            &client,
            ether_contract::Data::TAG,
            Some(UiDataSliceConfig { offset: 0, length: 0 }),
        )?;
        for (pubkey, _account) in contracts_v2 {
            contracts_v1_map.remove(&pubkey);
        }

        if contracts_v1_info.len() == 0 {
            return Ok(());
        }
    }
}
