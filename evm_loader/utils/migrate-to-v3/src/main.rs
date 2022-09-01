#![allow(deprecated)]

use std::env::current_dir;
use std::fs::File;
use std::io::Write;
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use evm_loader::account::{AccountData, ether_account, ether_contract, Packable};
use rustc_hash::FxHashMap;
use serde_json::{json, Value};
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter;
use solana_client::rpc_filter::{MemcmpEncodedBytes, RpcFilterType};
use solana_program::hash::Hash;
use solana_sdk::account::{Account, ReadableAccount};
use solana_sdk::account_info::IntoAccountInfo;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::{Keypair, read_keypair_file};
use solana_sdk::signer::Signer;
use solana_sdk::system_program;
use solana_sdk::transaction::Transaction;

use crate::json_rpc::{JsonRpcClient, Request};

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
    show_errors: bool,
    skip_preflight: bool,
    max_tps: usize,
    skip_backup: bool,
}

type AccountsMap<'a> = FxHashMap<Pubkey, Option<Pubkey>>;
type EthereumAccountV2<'a> = AccountData<'a, ether_account::DataV2>;

lazy_static::lazy_static! {
    static ref CONFIG: Config = serde_json::from_reader(
        File::open("config.json").expect("Failed to open `config.json` file"),
    ).expect("Failed to parse configuration file");
    static ref EVM_LOADER: Pubkey = Pubkey::from_str(&CONFIG.evm_loader_pubkey)
        .expect("Failed to parse `evm_loader_pubkey` in config");
    static ref PAYER: Keypair = read_keypair_file("payer.keys.json")
        .expect("Failed to read `payer.keys.json` file");
    static ref EXCLUDE_V2_ACCOUNTS: Vec<Pubkey> = vec![
        // pubkey!("74gQvu6R5DnSFdJ9JoMXFzk3e7uZgo9cZKxrdZBW8RaH"),
        // pubkey!("9HYmDSLt1svoJB23CkEZ9iMUCRUoNVj7iUS7T6pHPYr5"),
    ];
}

struct RecentBlockHash<'a> {
    client: &'a RpcClient,
    hash: Hash,
    time: Instant,
    recent_block_hash_ttl_sec: u64,
}

impl <'a> RecentBlockHash<'a> {
    fn new(client: &'a RpcClient, recent_block_hash_ttl_sec: u64) -> Self {
        Self {
            client,
            hash: Hash::new_from_array([0; 32]),
            time: Instant::now().sub(Duration::from_secs(60 * 60 * 24)),
            recent_block_hash_ttl_sec,
        }
    }

    fn get(&mut self) -> &Hash {
        if Instant::now().duration_since(self.time).as_secs() > self.recent_block_hash_ttl_sec {
            match self.client.get_latest_blockhash_with_commitment(CommitmentConfig::confirmed()) {
                Ok((hash, _)) => {
                    self.hash = hash;
                    self.time = Instant::now();
                    println!("New recent block hash: {}", self.hash);
                },
                Err(err) =>
                    println!("Failed to get recent blockhash: {:?}, using old value: {}", err, self.hash),
            }
        }

        &self.hash
    }
}

struct Batch<'url> {
    client: JsonRpcClient<'url>,
    batch: Vec<Request>,
    batch_size: usize,
    show_errors: bool,
    skip_preflight: bool,
    max_tps: usize,
    created_at: Instant,
    transaction_count: usize,
}

impl<'url> Batch<'url> {
    pub fn new(
        client: JsonRpcClient<'url>,
        batch_size: usize,
        show_errors: bool,
        skip_preflight: bool,
        max_tps: usize,
    ) -> Self {
        Self {
            client,
            batch: Vec::with_capacity(batch_size),
            batch_size,
            show_errors,
            skip_preflight,
            max_tps,
            created_at: Instant::now(),
            transaction_count: 0,
        }
    }

    pub fn send(&mut self) {
        if self.batch.is_empty() {
            return;
        }

        let next_transaction_at = self.created_at.add(
            Duration::from_secs_f64(self.transaction_count as f64 / self.max_tps as f64)
        );
        while next_transaction_at > Instant::now() {
            sleep(Duration::from_millis(10));
        }

        let now = Instant::now();
        if now - self.created_at > Duration::from_secs(20) {
            self.created_at = now;
            self.transaction_count = 0;
        }

        print!("Sending batch of {} requests... ", self.batch.len());
        if self.show_errors {
            println!();
        }
        match self.client.send_batch(&self.batch) {
            Ok(Value::Array(responses)) => {
                let mut error_count = 0;
                for response in responses {
                    if let Value::String(ref error_message) = response["error"]["message"] {
                        if self.show_errors {
                            println!("Error: {}", error_message);
                        }
                        error_count += 1;
                    }
                }
                if error_count == 0 {
                    println!("OK")
                } else {
                    println!("{} error(s)", error_count);
                }
            }
            Ok(response) => println!("Error: {:?}", response),
            Err(error) => println!("Error: {:?}", error),
        }

        self.transaction_count += self.batch.len();
        self.batch.clear();
    }

    pub fn add(&mut self, transaction: &Transaction) {
        let serialized = bincode::serialize(transaction)
            .expect("Transaction serialization error");
        let encoded = base64::encode(serialized);
        let request = self.client.request(
            "sendTransaction",
            json!([
                encoded,
                {
                    "skipPreflight": self.skip_preflight,
                    "preflightCommitment": "confirmed",
                    "encoding": "base64",
                },
            ])
        );
        self.batch.push(request);
        if self.batch.len() >= self.batch_size {
            self.send();
        }
    }
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

fn migrate_v2_to_v3_instruction(
    ether_account: Pubkey,
    ether_contract: Option<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(PAYER.pubkey(), true),                  // Funding account
        AccountMeta::new_readonly(system_program::id(), false),  // System program
        AccountMeta::new(ether_account, false),                  // Ether account
    ];
    if let Some(contract) = ether_contract {
        accounts.push(AccountMeta::new(contract, false));  // Ether contract
    }
    Instruction::new_with_bytes(*EVM_LOADER, &[0x21_u8], accounts)
}

fn make_migrate_v2_to_v3_transaction(
    ether_account: Pubkey,
    ether_contract: Option<Pubkey>,
    recent_blockhash: &Hash,
) -> Transaction {
    let instructions = vec![
        migrate_v2_to_v3_instruction(ether_account, ether_contract),
    ];
    let mut message = Message::new(&instructions, Some(&PAYER.pubkey()));
    message.recent_blockhash = *recent_blockhash;
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[&*PAYER], *recent_blockhash);

    transaction
}

fn convert_accounts_to_v3(
    batch: &mut Batch,
    recent_block_hash: &mut RecentBlockHash,
    to_convert: &AccountsMap,
) -> Result<()> {
    for (ether_account, ether_contract) in to_convert {
        batch.add(
            &make_migrate_v2_to_v3_transaction(
                *ether_account,
                *ether_contract,
                recent_block_hash.get(),
            ),
        );
    }

    Ok(())
}

fn main() -> Result<()> {
    println!("Payer public key: {}", PAYER.pubkey());

    let client = RpcClient::new_with_timeout(
        &CONFIG.url,
        Duration::from_secs(CONFIG.client_timeout_sec),
    );

    print!("Querying V2 accounts... ");
    let v2_accounts = get_evm_accounts(&client, ether_account::DataV2::TAG, None)?;
    println!("Queried {} accounts.", v2_accounts.len());

    print!("Querying Contract V2 accounts... ");
    let contract_v2_accounts = get_evm_accounts(&client, ether_contract::DataV2::TAG, None)?;
    println!("Queried {} accounts.", contract_v2_accounts.len());

    if !CONFIG.skip_backup {
        let path = current_dir()?
            .join("backups")
            .join(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs()
                    .to_string(),
            );
        std::fs::create_dir_all(&path)?;
        print!("Backing up to {:?}... ", path);
        for (pubkey, account_info) in v2_accounts.iter().chain(contract_v2_accounts.iter()) {
            std::fs::write(
                path.join(pubkey.to_string()),
                account_info.data(),
            )?;
        }
        println!("Backup completed. ");
    }

    drop(contract_v2_accounts);

    print!("Transforming... ");

    let mut to_convert: AccountsMap = v2_accounts.into_iter()
        .map(|(pubkey, mut account)| {
            let info = (&pubkey, &mut account).into_account_info();
            let account_data = EthereumAccountV2::from_account(&EVM_LOADER, &info)
                .unwrap_or_else(|err|
                    panic!("Cannot decode account V2 data for account: {}, error: {:?}", info.key, err)
                );
            (pubkey, account_data.code_account)
        })
        .collect();

    for exclude_pubkey in EXCLUDE_V2_ACCOUNTS.iter() {
        to_convert.remove(exclude_pubkey);
    }
    println!("OK ({} accounts)", to_convert.len());

    let mut recent_block_hash = RecentBlockHash::new(&client, CONFIG.recent_block_hash_ttl_sec);
    loop {
        println!("Accounts to convert: {}", to_convert.len());

        let mut batch = Batch::new(
            JsonRpcClient::new(&CONFIG.url),
            CONFIG.batch_size,
            CONFIG.show_errors,
            CONFIG.skip_preflight,
            CONFIG.max_tps,
        );

        println!("Converting accounts from V2 to V3...");

        convert_accounts_to_v3(
            &mut batch,
            &mut recent_block_hash,
            &to_convert,
        )?;

        batch.send();

        print!("Querying converted accounts... ");
        let v3_accounts = get_evm_accounts(
            &client,
            ether_account::Data::TAG,
            Some(UiDataSliceConfig { offset: 0, length: 0 }),
        )?;
        println!("OK ({} accounts)", v3_accounts.len());

        print!("Removing converted accounts... ");
        let mut removed = 0;
        for (pubkey, _account) in &v3_accounts {
            if to_convert.remove(pubkey).is_some() {
                removed += 1;
            }
        }
        println!("{} accounts removed", removed);

        if to_convert.is_empty() {
            println!("Conversion completed.");
            return Ok(());
        }
    }
}
