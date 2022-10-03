use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use log::{debug, info, trace, warn};
use evm::{H160, U256, H256};
use solana_sdk::{
    account::Account,
    account_info::AccountInfo,
    pubkey::{Pubkey},
    pubkey,
    sysvar::recent_blockhashes,
};
use solana_sdk::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use evm_loader::{
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    executor::{Action, OwnedAccountInfo, OwnedAccountInfoPartial},
    account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumStorage},
    account_storage::{AccountStorage}, precompile::is_precompile_address,
};
use evm_loader::account::ether_contract;
use evm_loader::account_storage::{AccountOperation, AccountsOperations};


use crate::Config;

const FAKE_OPERATOR: Pubkey = pubkey!("neonoperator1111111111111111111111111111111");

fn serde_pubkey_bs58<S>(value: &Pubkey, s: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
    let bs58 = bs58::encode(value).into_string();
    s.serialize_str(&bs58)
}

#[derive(serde::Serialize, Clone)]
pub struct NeonAccount {
    address: H160,
    #[serde(serialize_with = "serde_pubkey_bs58")]
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
    pub fn rpc_load(config: &Config, address: H160, writable: bool) -> Self {
        let (key, _) = make_solana_program_address(&address, &config.evm_loader);
        info!("get_account_from_solana 0x{} => {}", address, key);

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
    pub accounts: RefCell<HashMap<H160, NeonAccount>>,
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

    pub fn get_account_from_solana(config: &'a Config, address: &H160) -> (Pubkey, Option<Account>) {
        let (solana_address, _solana_nonce) = make_solana_program_address(address, &config.evm_loader);
        info!("get_account_from_solana 0x{} => {}", address, solana_address);

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

    fn add_ethereum_account(&self, address: &H160, writable: bool) -> bool {
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

    pub fn apply_actions(&self, actions: Vec<Action>) {
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
                Action::EvmLog { .. } => {},
                Action::EvmSetStorage { address, key, .. } => {
                    if key < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                        self.add_ethereum_account(&address, true);
                    } else {
                        let index = key & !U256::from(0xFF);

                        let (storage_account, _) = self.get_storage_address(&address, &index);
                        self.add_solana_account(storage_account, true);
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
                Action::ExternalInstruction { program_id, accounts, .. } => {
                    self.add_solana_account(program_id, false);

                    for account in &accounts {
                        self.add_solana_account(account.key, account.is_writable);
                    }
                }
            }
        }
    }

    pub fn apply_accounts_operations(&self, operations: AccountsOperations) {
        let mut accounts = self.accounts.borrow_mut();
        for (address, operation) in operations {
            let new_size = match operation {
                AccountOperation::Create { space } => space,
                AccountOperation::Resize { to, .. } => to,
            };
            accounts.entry(address).and_modify(|a| {
                a.size = new_size;
                a.additional_resize_steps = a.additional_resize_steps.max(
                    new_size
                        .saturating_sub(a.size_current)
                        .saturating_sub(1)
                        / MAX_PERMITTED_DATA_INCREASE,
                );
            });
        }
    }

    fn ethereum_account_map_or<F, R>(&self, address: &H160, default: R, f: F) -> R
    where 
        F: FnOnce(&EthereumAccount) -> R
    {
        self.add_ethereum_account(address, false);

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).unwrap();

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(&solana_account.account, account_data);
            let ethereum_account = EthereumAccount::from_account(&self.config.evm_loader, &info).unwrap();
            f(&ethereum_account)
        } else {
            default
        }
    }

    fn ethereum_contract_map_or<F, R>(&self, address: &H160, default: R, f: F) -> R
    where
        F: FnOnce(ether_contract::ContractData) -> R
    {
        self.add_ethereum_account(address, false);

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).unwrap();

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(&solana_account.account, account_data);
            let ethereum_account = EthereumAccount::from_account(&self.config.evm_loader, &info).unwrap();
            ethereum_account.contract_data().map_or(default, f)
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
        self.block_timestamp.into()
    }

    fn block_hash(&self, number: U256) -> H256 { 
        info!("block_hash {}", number);

        self.add_solana_account(recent_blockhashes::ID, false);

        if self.block_number <= number.as_u64() {
            return H256::default();
        }

        if let Ok(timestamp) = self.config.rpc_client.get_block(number.as_u64()) {
            H256::from_slice(&bs58::decode(timestamp.blockhash).into_vec().unwrap())
        } else {
            warn!("Got error trying to get block hash");
            H256::default()
        }
    }

    fn exists(&self, address: &H160) -> bool {
        info!("exists {}", address);

        self.add_ethereum_account(address, false);

        let accounts = self.accounts.borrow();
        accounts.contains_key(address)
    }

    fn nonce(&self, address: &H160) -> U256 {
        info!("nonce {}", address);

        self.ethereum_account_map_or(address, 0_u64, |a| a.trx_count).into()
    }

    fn balance(&self, address: &H160) -> U256 {
        info!("balance {}", address);

        self.ethereum_account_map_or(address, U256::zero(), |a| a.balance)
    }

    fn code_size(&self, address: &H160) -> usize {
        info!("code_size {}", address);
        self.ethereum_account_map_or(address, 0, |a| a.code_size as usize)
    }

    fn code_hash(&self, address: &H160) -> H256 {
        info!("code_hash {}", address);

        self.ethereum_contract_map_or(address,
            H256::default(), 
            |c| evm_loader::utils::keccak256_h256(&c.code())
        )
    }

    fn code(&self, address: &H160) -> Vec<u8> {
        info!("code {}", address);

        self.ethereum_contract_map_or(
            address,
            Vec::new(),
            |c| c.code().to_vec(),
        )
    }

    fn valids(&self, address: &H160) -> Vec<u8> {
        info!("valids {}", address);

        self.ethereum_contract_map_or(
            address,
            Vec::new(),
            |c| c.valids().to_vec()
        )
    }

    fn generation(&self, address: &H160) -> u32 {
        info!("generation {}", address);
        let value = self.ethereum_account_map_or(
            address,
            0_u32, 
            |c| c.generation,
        );

        info!("account generation {:?} - {:?}", address, value);
        value
    }

    fn storage(&self, address: &H160, index: &U256) -> U256 {
        debug!("storage {} -> {}", address, index);

        let value = if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            self.ethereum_contract_map_or(
                address,
                U256::zero(),
                |c| U256::from_big_endian(&c.storage()[index..index+32]),
            )
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let subindex = (*index & U256::from(0xFF)).as_u64() as u8;
            let index = *index & !U256::from(0xFF);
            
            let (solana_address, _) = self.get_storage_address(address, &index);
            debug!("read storage solana address {:?} - {:?}", address, solana_address);

            self.add_solana_account(solana_address, false);

            let rpc_response = self.config.rpc_client.get_account_with_commitment(
                &solana_address,
                self.config.rpc_client.commitment(),
            ).expect("Error querying account from Solana");
        
            if let Some(mut account) = rpc_response.value {
                if solana_sdk::system_program::check_id(&account.owner) {
                    debug!("read storage system owned");
                    U256::zero()
                } else {
                    let account_info = account_info(&solana_address, &mut account);
                    let storage = EthereumStorage::from_account(&self.config.evm_loader, &account_info).unwrap();
                    storage.get(subindex)
                }
            } else {
                debug!("storage account doesn't exist");
                U256::zero()
            }
        };

        debug!("Storage read {:?} -> {} = {}", address, index, value);

        value
    }

    fn solana_account_space(&self, address: &H160) -> Option<usize> {
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
    
            OwnedAccountInfo::from_account_info(&info)
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
    ether_address: &H160,
    program_id: &Pubkey
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], ether_address.as_bytes()], program_id)
}
