use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    str::FromStr,
    convert::TryInto,
};

use log::{debug, info, trace, warn};
use ethnum::U256;
use solana_sdk::{
    account::Account,
    account_info::AccountInfo,
    pubkey::{Pubkey},
    pubkey,
    sysvar::{recent_blockhashes, Sysvar}, rent::Rent,
};
use solana_sdk::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use evm_loader::{
    config::{STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT},
    executor::{Action, OwnedAccountInfo, OwnedAccountInfoPartial},
    account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumStorage, ether_storage::EthereumStorageAddress},
    account_storage::{AccountStorage}, evm::is_precompile_address,
    types::Address,
    gasometer::LAMPORTS_PER_SIGNATURE
};
use evm_loader::account::ether_contract;
use evm_loader::account_storage::{AccountOperation, AccountsOperations};

use crate::Config;

const FAKE_OPERATOR: Pubkey = pubkey!("neonoperator1111111111111111111111111111111");

fn serde_pubkey_bs58<S>(value: &Pubkey, s: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
    let bs58 = bs58::encode(value).into_string();
    s.serialize_str(&bs58)
}

#[allow(unused)]
fn deserialize_pubkey_from_str<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
    where
        D: serde::de::Deserializer<'de>,
{
    struct StringVisitor;
    impl<'de> serde::de::Visitor<'de> for StringVisitor {
        type Value = Pubkey;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string containing json data")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
        {
            Pubkey::from_str(v).map_err(E::custom)
        }
    }
    deserializer.deserialize_any(StringVisitor)
}


#[derive(serde::Serialize, Clone)]
pub struct NeonAccount {
    address: Address,
    #[serde(serialize_with = "serde_pubkey_bs58")]
    #[serde(deserialize_with = "deserialize_pubkey_from_str")]
    account: Pubkey,
    writable: bool,
    new: bool,
    size: usize,
    size_current: usize,
    additional_resize_steps: usize,
    #[serde(skip)]
    data: Option<Account>,
}

impl NeonAccount {
    pub fn rpc_load(config: &Config, address: Address, writable: bool) -> Self {
        let (key, _) = make_solana_program_address(&address, &config.evm_loader);
        info!("get_account_from_solana {} => {}", address, key);

        if let Ok(account) = config.rpc_client.get_account(&key) {
            trace!("Account found");

            Self {
                address,
                account: key,
                writable,
                new: false,
                size: account.data.len(),
                size_current: account.data.len(),
                additional_resize_steps: 0,
                data: Some(account)
            }
        }
        else {
            warn!("Account not found {}", address);

            Self {
                address, 
                account: key, 
                writable,
                new: true,
                size: 0,
                size_current: 0,
                additional_resize_steps: 0,
                data: None
            }
        }
    }
}

#[derive(serde::Serialize, Clone)]
pub struct SolanaAccount {
    #[serde(serialize_with = "serde_pubkey_bs58")]
    pubkey: Pubkey,
    is_writable: bool
}


#[allow(clippy::module_name_repetitions)]
pub struct EmulatorAccountStorage<'a> {
    pub accounts: RefCell<HashMap<Address, NeonAccount>>,
    pub solana_accounts: RefCell<HashMap<Pubkey, SolanaAccount>>,
    config: &'a Config,
    block_number: u64,
    block_timestamp: i64,
    neon_token_mint: Pubkey,
    chain_id: u64,
}

impl<'a> EmulatorAccountStorage<'a> {
    pub fn new(config: &'a Config, token_mint: Pubkey, chain_id: u64) -> EmulatorAccountStorage {
        trace!("backend::new");

        let slot = config.rpc_client.get_slot().unwrap_or_default();
        let timestamp = config.rpc_client.get_block_time(slot).unwrap_or_default();

        Self {
            accounts: RefCell::new(HashMap::new()),
            solana_accounts: RefCell::new(HashMap::new()),
            config,
            block_number: slot,
            block_timestamp: timestamp,
            neon_token_mint: token_mint,
            chain_id,
        }
    }

    pub fn get_account_from_solana(config: &'a Config, address: &Address) -> (Pubkey, Option<Account>) {
        let (solana_address, _solana_nonce) = make_solana_program_address(address, &config.evm_loader);
        info!("get_account_from_solana {} => {}", address, solana_address);

        if let Ok(acc) = config.rpc_client.get_account(&solana_address) {
            trace!("Account found");
            trace!("Account data len {}", acc.data.len());
            trace!("Account owner {}", acc.owner);

            (solana_address, Some(acc))
        } else {
            warn!("Account not found {}", address);

            (solana_address, None)
        }
    }

    fn add_ethereum_account(&self, address: &Address, writable: bool) -> bool {
        if is_precompile_address(address) {
            return true;
        }

        let mut accounts = self.accounts.borrow_mut();

        if let Some(ref mut account) = accounts.get_mut(address) {
            account.writable |= writable;

            true
        } else {
            let account = NeonAccount::rpc_load(self.config, *address, writable);
            accounts.insert(*address, account);

            false
        }
    }

    fn add_solana_account(&self, pubkey: Pubkey, is_writable: bool) {
        if solana_sdk::system_program::check_id(&pubkey) {
            return;
        }

        if pubkey == FAKE_OPERATOR {
            return;
        }

        let mut solana_accounts = self.solana_accounts.borrow_mut();
        
        let account = SolanaAccount { pubkey, is_writable };
        if is_writable {
            solana_accounts.insert(pubkey, account);
        } else {
            solana_accounts.entry(pubkey).or_insert(account);
        }
    }

    #[must_use]
    pub fn apply_actions(&self, actions: Vec<Action>) -> u64 {
        let mut gas = 0_u64;
        let rent = Rent::get().expect("Rent get error");

        for action in actions {
            #[allow(clippy::match_same_arms)]
            match action {
                Action::NeonTransfer { source, target, .. } => {
                    self.add_ethereum_account(&source, true);
                    self.add_ethereum_account(&target, true);
                },
                Action::NeonWithdraw { source, .. } => {
                    self.add_ethereum_account(&source, true);
                },
                Action::EvmSetStorage { address, index, value } => {
                    if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                        self.add_ethereum_account(&address, true);
                    } else {
                        let (base, _) = address.find_solana_address(self.program_id());
                        let storage_account = EthereumStorageAddress::new(self.program_id(), &base, &index);
                        self.add_solana_account(*storage_account.pubkey(), true);

                        if self.storage(&address, &index) == [0_u8; 32] {
                            let metadata_size = EthereumStorage::SIZE;
                            let element_size = 1 + std::mem::size_of_val(&value);

                            let cost = rent.minimum_balance(metadata_size + element_size);
                            gas = gas.saturating_add(cost);
                        }
                    }
                },
                Action::EvmIncrementNonce { address } => {
                    self.add_ethereum_account(&address, true);
                },
                Action::EvmSetCode { address, .. } => {
                    self.add_ethereum_account(&address, true);
                },
                Action::EvmSelfDestruct { address } => {
                    self.add_ethereum_account(&address, true);
                },
                Action::ExternalInstruction { program_id, accounts, allocate, .. } => {
                    self.add_solana_account(program_id, false);

                    for account in accounts {
                        self.add_solana_account(account.pubkey, account.is_writable);
                    }

                    if allocate > 0 {
                        let cost = rent.minimum_balance(allocate);
                        gas = gas.saturating_add(cost);
                    }
                }
            }
        }

        gas
    }

    #[must_use]
    pub fn apply_accounts_operations(&self, operations: AccountsOperations) -> u64 {
        let mut gas = 0_u64;
        let rent = Rent::get().expect("Rent get error");

        let mut iterations = 0_usize;

        let mut accounts = self.accounts.borrow_mut();
        for (address, operation) in operations {
            let new_size = match operation {
                AccountOperation::Create { space } => space,
                AccountOperation::Resize { to, .. } => to,
            };
            accounts.entry(address).and_modify(|a| {
                a.size = new_size;
                a.additional_resize_steps = new_size
                    .saturating_sub(a.size_current)
                    .saturating_sub(1)
                    / MAX_PERMITTED_DATA_INCREASE;
                iterations = iterations.max(a.additional_resize_steps);
            });

            let allocate_cost = rent.minimum_balance(new_size);
            gas = gas.saturating_add(allocate_cost);
        }

        let iterations_cost = (iterations as u64) * LAMPORTS_PER_SIGNATURE;

        gas.saturating_add(iterations_cost)
    }

    fn ethereum_account_map_or<F, R>(&self, address: &Address, default: R, f: F) -> R
    where 
        F: FnOnce(&EthereumAccount) -> R
    {
        self.add_ethereum_account(address, false);

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).expect("get account error");

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(&solana_account.account, account_data);
            EthereumAccount::from_account(&self.config.evm_loader, &info)
                .map_or(default, |a| f(&a))
        } else {
            default
        }
    }

    fn ethereum_contract_map_or<F, R>(&self, address: &Address, default: R, f: F) -> R
    where
        F: FnOnce(ether_contract::ContractData) -> R
    {
        self.add_ethereum_account(address, false);

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).expect("get account error");

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(&solana_account.account, account_data);
            let account = EthereumAccount::from_account(&self.config.evm_loader, &info);
            match &account {
                Ok(a) => a.contract_data().map_or(default, f),
                Err(_) => default,
            }
        } else {
            default
        }
    }
}


impl<'a> AccountStorage for EmulatorAccountStorage<'a> {
    fn neon_token_mint(&self) -> &Pubkey {
        info!("neon_token_mint");
        &self.neon_token_mint
    }

    fn operator(&self) -> &Pubkey {
        info!("operator");
        &FAKE_OPERATOR
    }

    fn program_id(&self) -> &Pubkey {
        info!("program_id");
        &self.config.evm_loader
    }

    fn block_number(&self) -> U256 {
        info!("block_number");
        self.block_number.into()
    }

    fn block_timestamp(&self) -> U256 {
        info!("block_timestamp");
        self.block_timestamp.try_into().unwrap()
    }

    fn block_hash(&self, number: U256) -> [u8; 32] { 
        info!("block_hash {}", number);

        self.add_solana_account(recent_blockhashes::ID, false);

        if self.block_number <= number.as_u64() {
            return <[u8; 32]>::default();
        }

        if let Ok(timestamp) = self.config.rpc_client.get_block(number.as_u64()) {
            let hash = bs58::decode(timestamp.blockhash).into_vec().unwrap();
            hash.try_into().unwrap()
        } else {
            warn!("Got error trying to get block hash");
            <[u8; 32]>::default()
        }
    }

    fn exists(&self, address: &Address) -> bool {
        info!("exists {}", address);

        self.add_ethereum_account(address, false);

        let accounts = self.accounts.borrow();
        accounts.contains_key(address)
    }

    fn nonce(&self, address: &Address) -> u64 {
        info!("nonce {}", address);

        self.ethereum_account_map_or(address, 0_u64, |a| a.trx_count)
    }

    fn balance(&self, address: &Address) -> U256 {
        info!("balance {}", address);

        self.ethereum_account_map_or(address, U256::ZERO, |a| a.balance)
    }

    fn code_size(&self, address: &Address) -> usize {
        info!("code_size {}", address);
        self.ethereum_account_map_or(address, 0, |a| a.code_size as usize)
    }

    fn code_hash(&self, address: &Address) -> [u8; 32] {
        use solana_sdk::keccak::hash;

        info!("code_hash {}", address);

        self.ethereum_contract_map_or(address,
            <[u8; 32]>::default(), 
            |c| hash(&c.code()).to_bytes()
        )
    }

    fn code(&self, address: &Address) -> evm_loader::evm::Buffer {
        use evm_loader::evm::Buffer;

        info!("code {}", address);

        self.ethereum_contract_map_or(
            address,
            Buffer::empty(),
            |c| Buffer::new(&c.code()),
        )
    }

    fn generation(&self, address: &Address) -> u32 {
        let value = self.ethereum_account_map_or(
            address,
            0_u32, 
            |c| c.generation,
        );

        info!("account generation {:?} - {:?}", address, value);
        value
    }

    fn storage(&self, address: &Address, index: &U256) -> [u8; 32] {
        let value = if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            self.ethereum_contract_map_or(
                address,
                <[u8; 32]>::default(),
                |c| c.storage()[index..index+32].try_into().unwrap(),
            )
        } else {
            let subindex = (index & 0xFF).as_u8();
            let index = index & !U256::new(0xFF);
            
            let (base, _) = address.find_solana_address(self.program_id());
            let solana_address = *EthereumStorageAddress::new(self.program_id(), &base, &index).pubkey();
            debug!("read storage solana address {:?} - {:?}", address, solana_address);

            self.add_solana_account(solana_address, false);

            let rpc_response = self.config.rpc_client.get_account_with_commitment(
                &solana_address,
                self.config.rpc_client.commitment(),
            ).expect("Error querying account from Solana");
        
            if let Some(mut account) = rpc_response.value {
                if solana_sdk::system_program::check_id(&account.owner) {
                    debug!("read storage system owned");
                    <[u8; 32]>::default()
                } else {
                    let account_info = account_info(&solana_address, &mut account);
                    let storage = EthereumStorage::from_account(&self.config.evm_loader, &account_info).expect("EthereumAccount ctor error");
                    if (storage.address != *address) || (storage.index != index) || (storage.generation != self.generation(address)) {
                        debug!("storage collision");
                        <[u8; 32]>::default()
                    } else {
                        storage.get(subindex)
                    }
                }
            } else {
                debug!("storage account doesn't exist");
                <[u8; 32]>::default()
            }
        };

        debug!("Storage read {:?} -> {} = {}", address, index, hex::encode(value));

        value
    }

    fn solana_account_space(&self, address: &Address) -> Option<usize> {
        self.ethereum_account_map_or(address, None, |account| Some(account.info.data_len()))
    }

    fn chain_id(&self) -> u64 {
        info!("chain_id");

        self.chain_id
    }

    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        info!("clone_solana_account {}", address);

        if address == &FAKE_OPERATOR {
            OwnedAccountInfo {
                key: FAKE_OPERATOR,
                is_signer: true,
                is_writable: false,
                lamports: 100 * 1_000_000_000,
                data: vec![],
                owner: solana_sdk::system_program::ID,
                executable: false,
                rent_epoch: 0,
            }
        } else {
            self.add_solana_account(*address, false);

            let mut account = self.config.rpc_client.get_account(address).unwrap_or_default();
            let info = account_info(address, &mut account);
    
            OwnedAccountInfo::from_account_info(self.program_id(), &info)
        }
    }

    fn clone_solana_account_partial(&self, address: &Pubkey, offset: usize, len: usize) -> Option<OwnedAccountInfoPartial> {
        info!("clone_solana_account_partial {}", address);

        let account = self.clone_solana_account(address);

        Some(OwnedAccountInfoPartial {
            key: account.key,
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            lamports: account.lamports,
            data: account.data.get(offset .. offset + len).map(<[u8]>::to_vec)?,
            data_offset: offset,
            data_total_len: account.data.len(),
            owner: account.owner,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
        })
    }
}


/// Creates new instance of `AccountInfo` from `Account`.
pub fn account_info<'a>(key: &'a Pubkey, account: &'a mut Account) -> AccountInfo<'a> {
    AccountInfo {
        key,
        is_signer: false,
        is_writable: false,
        lamports: Rc::new(RefCell::new(&mut account.lamports)),
        data: Rc::new(RefCell::new(&mut account.data)),
        owner: &account.owner,
        executable: account.executable,
        rent_epoch: account.rent_epoch,
    }
}

pub fn make_solana_program_address(
    ether_address: &Address,
    program_id: &Pubkey
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], ether_address.as_bytes()], program_id)
}
