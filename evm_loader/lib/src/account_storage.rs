use async_trait::async_trait;
use std::{cell::RefCell, collections::HashMap, convert::TryInto, rc::Rc};

use crate::{rpc::Rpc, NeonError};
use ethnum::U256;
use evm_loader::account::ether_contract;
use evm_loader::account_storage::{find_slot_hash, AccountOperation, AccountsOperations};
use evm_loader::evm::tracing::{AccountOverrides, BlockOverrides};
use evm_loader::evm::Buffer;
use evm_loader::{
    account::{
        ether_storage::EthereumStorageAddress, EthereumAccount, EthereumStorage,
        ACCOUNT_SEED_VERSION,
    },
    account_storage::AccountStorage,
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    executor::{Action, OwnedAccountInfo},
    gasometer::LAMPORTS_PER_SIGNATURE,
    types::Address,
};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use solana_client::client_error;
use solana_sdk::entrypoint::MAX_PERMITTED_DATA_INCREASE;
use solana_sdk::{
    account::Account,
    account_info::AccountInfo,
    commitment_config::CommitmentConfig,
    pubkey,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{slot_hashes, Sysvar},
};

use crate::types::PubkeyBase58;

const FAKE_OPERATOR: Pubkey = pubkey!("neonoperator1111111111111111111111111111111");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeonAccount {
    address: Address,
    account: PubkeyBase58,
    writable: bool,
    new: bool,
    size: usize,
    size_current: usize,
    additional_resize_steps: usize,
    #[serde(skip)]
    data: Option<Account>,
}

impl NeonAccount {
    fn new(address: Address, pubkey: Pubkey, account: Option<Account>, writable: bool) -> Self {
        if let Some(account) = account {
            trace!("Account found {}", address);

            Self {
                address,
                account: pubkey.into(),
                writable,
                new: false,
                size: account.data.len(),
                size_current: account.data.len(),
                additional_resize_steps: 0,
                data: Some(account),
            }
        } else {
            trace!("Account not found {}", address);

            Self {
                address,
                account: pubkey.into(),
                writable,
                new: true,
                size: 0,
                size_current: 0,
                additional_resize_steps: 0,
                data: None,
            }
        }
    }

    pub async fn rpc_load(
        rpc_client: &dyn Rpc,
        evm_loader: &Pubkey,
        address: Address,
        writable: bool,
    ) -> Self {
        let (key, _) = make_solana_program_address(&address, evm_loader);
        info!("get_account_from_solana {address} => {key}");

        let account = match rpc_client.get_account(&key).await {
            Ok(account) => Some(account),
            Err(err) => {
                error!("rpc_client.get_account {key} error: {err:?}");
                None
            }
        };
        Self::new(address, key, account, writable)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaAccount {
    pubkey: PubkeyBase58,
    is_writable: bool,
    #[serde(skip)]
    data: Option<Account>,
}

#[allow(clippy::module_name_repetitions)]
pub struct EmulatorAccountStorage<'a> {
    pub accounts: RefCell<HashMap<Address, NeonAccount>>,
    pub solana_accounts: RefCell<HashMap<Pubkey, SolanaAccount>>,
    rpc_client: &'a dyn Rpc,
    evm_loader: Pubkey,
    block_number: u64,
    block_timestamp: i64,
    neon_token_mint: Pubkey,
    chain_id: u64,
    commitment: CommitmentConfig,
    state_overrides: Option<AccountOverrides>,
}

impl<'a> EmulatorAccountStorage<'a> {
    pub async fn new(
        rpc_client: &'a dyn Rpc,
        evm_loader: Pubkey,
        token_mint: Pubkey,
        chain_id: u64,
        commitment: CommitmentConfig,
        block_overrides: &Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
    ) -> Result<EmulatorAccountStorage<'a>, NeonError> {
        trace!("backend::new");

        let block_number = match block_overrides
            .as_ref()
            .and_then(|overrides| overrides.number)
        {
            None => rpc_client.get_slot().await?,
            Some(number) => number,
        };

        let block_timestamp = match block_overrides
            .as_ref()
            .and_then(|overrides| overrides.time)
        {
            None => rpc_client.get_block_time(block_number).await?,
            Some(time) => time,
        };

        Ok(Self {
            accounts: RefCell::new(HashMap::new()),
            solana_accounts: RefCell::new(HashMap::new()),
            rpc_client,
            evm_loader,
            block_number,
            block_timestamp,
            neon_token_mint: token_mint,
            chain_id,
            commitment,
            state_overrides,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn with_accounts(
        rpc_client: &'a dyn Rpc,
        evm_loader: Pubkey,
        token_mint: Pubkey,
        chain_id: u64,
        commitment: CommitmentConfig,
        accounts: &[Address],
        solana_accounts: &[Pubkey],
        block_overrides: &Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
    ) -> Result<EmulatorAccountStorage<'a>, NeonError> {
        let storage = Self::new(
            rpc_client,
            evm_loader,
            token_mint,
            chain_id,
            commitment,
            block_overrides,
            state_overrides,
        )
        .await?;
        storage
            .initialize_cached_accounts(accounts, solana_accounts)
            .await;

        Ok(storage)
    }

    pub async fn initialize_cached_accounts(
        &self,
        addresses: &[Address],
        solana_accounts: &[Pubkey],
    ) {
        let pubkeys: Vec<_> = addresses
            .iter()
            .map(|address| make_solana_program_address(address, &self.evm_loader).0)
            .chain(solana_accounts.iter().copied())
            .collect();

        if let Ok(accounts) = self.rpc_client.get_multiple_accounts(&pubkeys).await {
            let entries = addresses
                .iter()
                .zip(accounts.iter().take(addresses.len()))
                .zip(pubkeys.iter().take(addresses.len()));
            for ((&address, account), &pubkey) in entries {
                self.accounts.borrow_mut().insert(
                    address,
                    NeonAccount::new(address, pubkey, account.clone(), false),
                );
            }

            let entries = accounts.iter().skip(addresses.len()).zip(solana_accounts);
            let mut solana_accounts_storage = self.solana_accounts.borrow_mut();
            for (account, &pubkey) in entries {
                solana_accounts_storage.insert(
                    pubkey,
                    SolanaAccount {
                        pubkey: pubkey.into(),
                        is_writable: false,
                        data: account.clone(),
                    },
                );
            }
        }
    }

    pub async fn get_account(&self, pubkey: &Pubkey) -> client_error::Result<Option<Account>> {
        if let Some(account) = self.solana_accounts.borrow().get(pubkey) {
            if let Some(ref data) = account.data {
                return Ok(Some(data.clone()));
            }
        }

        let result = self
            .rpc_client
            .get_account_with_commitment(pubkey, self.commitment)
            .await?;

        self.solana_accounts
            .borrow_mut()
            .entry(*pubkey)
            .and_modify(|a| a.data = result.value.clone())
            .or_insert(SolanaAccount {
                pubkey: pubkey.into(),
                is_writable: false,
                data: result.value.clone(),
            });

        Ok(result.value)
    }

    pub async fn get_account_from_solana(
        rpc_client: &'a dyn Rpc,
        evm_loader: &'a Pubkey,
        address: &Address,
    ) -> (Pubkey, Option<Account>) {
        let (solana_address, _solana_nonce) = make_solana_program_address(address, evm_loader);
        info!("get_account_from_solana {} => {}", address, solana_address);

        if let Ok(acc) = rpc_client.get_account(&solana_address).await {
            trace!("Account found");
            trace!("Account data len {}", acc.data.len());
            trace!("Account owner {}", acc.owner);

            (solana_address, Some(acc))
        } else {
            warn!("Account not found {}", address);

            (solana_address, None)
        }
    }

    async fn add_ethereum_account(&self, address: &Address, writable: bool) -> bool {
        if let Some(ref mut account) = self.accounts.borrow_mut().get_mut(address) {
            account.writable |= writable;

            return true;
        }

        let account =
            NeonAccount::rpc_load(self.rpc_client, &self.evm_loader, *address, writable).await;
        self.accounts.borrow_mut().insert(*address, account);

        false
    }

    async fn add_solana_account(&self, pubkey: Pubkey, is_writable: bool) {
        if solana_sdk::system_program::check_id(&pubkey) {
            return;
        }

        if pubkey == FAKE_OPERATOR {
            return;
        }

        let mut solana_accounts = self.solana_accounts.borrow_mut();

        let account = SolanaAccount {
            pubkey: pubkey.into(),
            is_writable,
            data: None,
        };
        if is_writable {
            solana_accounts
                .entry(pubkey)
                // If account is present in cache ensure the data is not lost
                .and_modify(|a| a.is_writable = true)
                .or_insert(account);
        } else {
            solana_accounts.entry(pubkey).or_insert(account);
        }
    }

    pub async fn apply_actions(&self, actions: &[Action]) -> u64 {
        info!("apply_actions");

        let mut gas = 0_u64;
        let rent = Rent::get().expect("Rent get error");

        for action in actions {
            #[allow(clippy::match_same_arms)]
            match action {
                Action::NeonTransfer {
                    source,
                    target,
                    value,
                } => {
                    info!("neon transfer {value} from {source} to {target}");

                    self.add_ethereum_account(source, true).await;
                    self.add_ethereum_account(target, true).await;
                }
                Action::NeonWithdraw { source, value } => {
                    info!("neon withdraw {value} from {source}");

                    self.add_ethereum_account(source, true).await;
                }
                Action::EvmSetStorage {
                    address,
                    index,
                    value,
                } => {
                    info!("set storage {address} -> {index} = {}", hex::encode(value));

                    if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                        self.add_ethereum_account(address, true).await;
                    } else {
                        let (base, _) = address.find_solana_address(self.program_id());
                        let storage_account =
                            EthereumStorageAddress::new(self.program_id(), &base, index);
                        self.add_solana_account(*storage_account.pubkey(), true)
                            .await;

                        if self.storage(address, index).await == [0_u8; 32] {
                            let metadata_size = EthereumStorage::SIZE;
                            let element_size = 1 + std::mem::size_of_val(value);

                            let cost = rent.minimum_balance(metadata_size + element_size);
                            gas = gas.saturating_add(cost);
                        }
                    }
                }
                Action::EvmIncrementNonce { address } => {
                    info!("nonce increment {address}");

                    self.add_ethereum_account(address, true).await;
                }
                Action::EvmSetCode { address, code } => {
                    info!("set code {address} -> {} bytes", code.len());

                    self.add_ethereum_account(address, true).await;
                }
                Action::EvmSelfDestruct { address } => {
                    info!("selfdestruct {address}");

                    self.add_ethereum_account(address, true).await;
                }
                Action::ExternalInstruction {
                    program_id,
                    accounts,
                    fee,
                    ..
                } => {
                    info!("external call {program_id}");

                    self.add_solana_account(*program_id, false).await;

                    for account in accounts {
                        self.add_solana_account(account.pubkey, account.is_writable)
                            .await;
                    }

                    gas = gas.saturating_add(*fee);
                }
            }
        }

        gas
    }

    pub async fn apply_accounts_operations(&self, operations: AccountsOperations) -> u64 {
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
                a.additional_resize_steps =
                    new_size.saturating_sub(a.size_current).saturating_sub(1)
                        / MAX_PERMITTED_DATA_INCREASE;
                iterations = iterations.max(a.additional_resize_steps);
            });

            let allocate_cost = rent.minimum_balance(new_size);
            gas = gas.saturating_add(allocate_cost);
        }

        let iterations_cost = (iterations as u64) * LAMPORTS_PER_SIGNATURE;

        gas.saturating_add(iterations_cost)
    }

    async fn ethereum_account_map_or<F, R>(&self, address: &Address, default: R, f: F) -> R
    where
        F: FnOnce(&EthereumAccount) -> R,
    {
        self.add_ethereum_account(address, false).await;

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).expect("get account error");

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(solana_account.account.as_ref(), account_data);
            EthereumAccount::from_account(&self.evm_loader, &info)
                .map(|mut ether_account| {
                    if let Some(account_overrides) = &self.state_overrides {
                        if let Some(account_override) = account_overrides.get(address) {
                            account_override.apply(&mut ether_account);
                        }
                    }
                    ether_account
                })
                .map_or(default, |a| f(&a))
        } else {
            default
        }
    }

    async fn ethereum_contract_map_or<F, R>(&self, address: &Address, default: R, f: F) -> R
    where
        F: FnOnce(ether_contract::ContractData) -> R,
    {
        self.add_ethereum_account(address, false).await;

        let mut accounts = self.accounts.borrow_mut();
        let solana_account = accounts.get_mut(address).expect("get account error");

        if let Some(account_data) = &mut solana_account.data {
            let info = account_info(solana_account.account.as_ref(), account_data);
            let account = EthereumAccount::from_account(&self.evm_loader, &info);
            match &account {
                Ok(a) => a.contract_data().map_or(default, f),
                Err(_) => default,
            }
        } else {
            default
        }
    }

    async fn get_code(&self, address: &Address) -> Buffer {
        self.ethereum_contract_map_or(address, Buffer::empty(), |c| {
            self.state_overrides
                .as_ref()
                .and_then(|account_overrides| account_overrides.get(address)?.code.as_ref())
                .map_or_else(
                    || Buffer::from_slice(&c.code()),
                    |code| Buffer::from_slice(&code.0),
                )
        })
        .await
    }
}

#[async_trait(?Send)]
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
        debug!("program_id");
        &self.evm_loader
    }

    fn block_number(&self) -> U256 {
        info!("block_number");
        self.block_number.into()
    }

    fn block_timestamp(&self) -> U256 {
        info!("block_timestamp");
        self.block_timestamp.try_into().unwrap()
    }

    async fn block_hash(&self, slot: u64) -> [u8; 32] {
        info!("block_hash {slot}");

        self.add_solana_account(slot_hashes::ID, false).await;

        if let Ok(Some(slot_hashes_account)) = self.get_account(&slot_hashes::ID).await {
            let slot_hashes_data = slot_hashes_account.data.as_slice();
            find_slot_hash(slot, slot_hashes_data)
        } else {
            panic!("Error querying account {} from Solana", slot_hashes::ID)
        }
    }

    async fn exists(&self, address: &Address) -> bool {
        info!("exists {address}");

        self.add_ethereum_account(address, false).await;

        let accounts = self.accounts.borrow();
        accounts.contains_key(address)
    }

    async fn nonce(&self, address: &Address) -> u64 {
        info!("nonce {address}");

        self.ethereum_account_map_or(address, 0_u64, |a| a.trx_count)
            .await
    }

    async fn balance(&self, address: &Address) -> U256 {
        info!("balance {address}");

        self.ethereum_account_map_or(address, U256::ZERO, |a| a.balance)
            .await
    }

    async fn code_size(&self, address: &Address) -> usize {
        info!("code_size {address}");

        self.ethereum_account_map_or(address, 0, |a| a.code_size as usize)
            .await
    }

    async fn code_hash(&self, address: &Address) -> [u8; 32] {
        use solana_sdk::keccak::hash;

        info!("code_hash {address}");

        // https://eips.ethereum.org/EIPS/eip-1052
        // https://eips.ethereum.org/EIPS/eip-161
        let is_non_existent_account = self
            .ethereum_account_map_or(address, true, |a| {
                a.trx_count == 0 && a.balance == 0 && a.code_size == 0
            })
            .await;

        if is_non_existent_account {
            return <[u8; 32]>::default();
        }

        // return empty hash(&[]) as a default value, or code's hash if contract exists
        hash(self.get_code(address).await.as_ref()).to_bytes()
    }

    async fn code(&self, address: &Address) -> Buffer {
        info!("code {address}");

        self.get_code(address).await
    }

    async fn generation(&self, address: &Address) -> u32 {
        let value = self
            .ethereum_account_map_or(address, 0_u32, |c| c.generation)
            .await;

        info!("account generation {address} - {value}");
        value
    }

    async fn storage(&self, address: &Address, index: &U256) -> [u8; 32] {
        if let Some(account_overrides) = &self.state_overrides {
            if let Some(account_override) = account_overrides.get(address) {
                match (&account_override.state, &account_override.state_diff) {
                    (None, None) => (),
                    (Some(_), Some(_)) => {
                        panic!("Account {address} has both `state` and `stateDiff` overrides")
                    }
                    (Some(state), None) => {
                        return state
                            .get(index)
                            .map(|value| value.to_be_bytes())
                            .unwrap_or_default()
                    }
                    (None, Some(state_diff)) => {
                        if let Some(value) = state_diff.get(index) {
                            return value.to_be_bytes();
                        }
                    }
                }
            }
        }
        let value = if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            self.ethereum_contract_map_or(address, <[u8; 32]>::default(), |c| {
                c.storage()[index..index + 32].try_into().unwrap()
            })
            .await
        } else {
            let subindex = (index & 0xFF).as_u8();
            let index = index & !U256::new(0xFF);

            let (base, _) = address.find_solana_address(self.program_id());
            let storage_address = EthereumStorageAddress::new(self.program_id(), &base, &index);

            self.add_solana_account(*storage_address.pubkey(), false)
                .await;

            let rpc_response = self
                .get_account(storage_address.pubkey())
                .await
                .expect("Error querying account from Solana");

            if let Some(mut account) = rpc_response {
                if solana_sdk::system_program::check_id(&account.owner) {
                    debug!("read storage system owned");
                    <[u8; 32]>::default()
                } else {
                    let account_info = account_info(storage_address.pubkey(), &mut account);
                    let storage = EthereumStorage::from_account(&self.evm_loader, &account_info)
                        .expect("EthereumAccount ctor error");
                    if (storage.address != *address)
                        || (storage.index != index)
                        || (storage.generation != self.generation(address).await)
                    {
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

        info!("storage {address} -> {index} = {}", hex::encode(value));

        value
    }

    async fn solana_account_space(&self, address: &Address) -> Option<usize> {
        self.ethereum_account_map_or(address, None, |account| Some(account.info.data_len()))
            .await
    }

    fn chain_id(&self) -> u64 {
        info!("chain_id");

        self.chain_id
    }

    async fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
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
            self.add_solana_account(*address, false).await;

            let mut account = self
                .get_account(address)
                .await
                .unwrap_or_default()
                .unwrap_or_default();
            let info = account_info(address, &mut account);

            OwnedAccountInfo::from_account_info(self.program_id(), &info)
        }
    }

    async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R,
    {
        self.add_solana_account(*address, false).await;

        let mut account = self
            .get_account(address)
            .await
            .unwrap_or_default()
            .unwrap_or_default();
        let info = account_info(address, &mut account);

        action(&info)
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

pub fn make_solana_program_address(ether_address: &Address, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&[ACCOUNT_SEED_VERSION], ether_address.as_bytes()],
        program_id,
    )
}
