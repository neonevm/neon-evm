use crate::account_data::AccountData;
use crate::commands::get_config::{BuildConfigSimulator, ChainInfo};
use crate::tracing::{AccountOverrides, BlockOverrides};
use crate::{rpc::Rpc, solana_simulator::SolanaSimulator, NeonError, NeonResult};
use async_trait::async_trait;
use elsa::FrozenMap;
use ethnum::U256;
use evm_loader::account_storage::LogCollector;
pub use evm_loader::account_storage::{AccountStorage, SyncedAccountStorage};
use evm_loader::{
    account::{
        legacy::{LegacyEtherData, LegacyStorageData},
        BalanceAccount, ContractAccount, StorageCell, StorageCellAddress,
    },
    account_storage::find_slot_hash,
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    error::Error as EvmLoaderError,
    executor::OwnedAccountInfo,
    types::Address,
};
use log::{debug, info, trace};
use solana_sdk::{
    account::Account,
    account_info::{AccountInfo, IntoAccountInfo},
    clock::Clock,
    instruction::Instruction,
    program_error::ProgramError,
    pubkey,
    pubkey::{Pubkey, PubkeyError},
    rent::Rent,
    system_program,
    sysvar::slot_hashes,
    transaction_context::TransactionReturnData,
};
use std::collections::{HashMap, HashSet};
use std::{
    cell::{Ref, RefCell, RefMut},
    convert::TryInto,
    rc::Rc,
};
use web3::types::Log;

const FAKE_OPERATOR: Pubkey = pubkey!("neonoperator1111111111111111111111111111111");

#[derive(Default, Clone, Copy)]
pub struct ExecuteStatus {
    pub external_solana_call: bool,
    pub reverts_before_solana_calls: bool,
    pub reverts_after_solana_calls: bool,
}

#[derive(Debug, Clone)]
pub struct SolanaAccount {
    pub pubkey: Pubkey,
    pub is_writable: bool,
    pub is_legacy: bool,
    pub lamports_after_upgrade: Option<u64>,
}

pub type SolanaOverrides = HashMap<Pubkey, Option<Account>>;

trait UpdateLamports<'a> {
    fn update_lamports(&mut self, rent: &Rent) {
        let required_lamports = rent.minimum_balance(self.required_lamports());
        if self.info().lamports() < required_lamports {
            let mut lamports = self.info().lamports.borrow_mut();
            **lamports = required_lamports;
        }
    }
    fn required_lamports(&self) -> usize;
    fn info(&self) -> &AccountInfo<'a>;
}
impl<'a> UpdateLamports<'a> for BalanceAccount<'a> {
    fn required_lamports(&self) -> usize {
        BalanceAccount::required_account_size()
    }
    fn info(&self) -> &AccountInfo<'a> {
        self.info()
    }
}
impl<'a> UpdateLamports<'a> for ContractAccount<'a> {
    fn required_lamports(&self) -> usize {
        ContractAccount::required_account_size(self.code().as_ref())
    }
    fn info(&self) -> &AccountInfo<'a> {
        self.info()
    }
}
impl<'a> UpdateLamports<'a> for StorageCell<'a> {
    fn required_lamports(&self) -> usize {
        StorageCell::required_account_size(self.cells().len())
    }
    fn info(&self) -> &AccountInfo<'a> {
        self.info()
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct EmulatorAccountStorage<'rpc, T: Rpc> {
    accounts: FrozenMap<Pubkey, Box<RefCell<AccountData>>>,
    call_stack: Vec<FrozenMap<Pubkey, Box<RefCell<AccountData>>>>,

    pub gas: u64,
    pub realloc_iterations: u64,
    pub execute_status: ExecuteStatus,
    rpc: &'rpc T,
    program_id: Pubkey,
    operator: Pubkey,
    chains: Vec<ChainInfo>,
    block_number: u64,
    block_timestamp: i64,
    timestamp_used: RefCell<bool>,
    rent: Rent,
    state_overrides: Option<AccountOverrides>,
    accounts_cache: FrozenMap<Pubkey, Box<Option<Account>>>,
    used_accounts: FrozenMap<Pubkey, Box<RefCell<SolanaAccount>>>,
    return_data: Option<TransactionReturnData>,
    logs: Vec<Log>,
    logs_stack: Vec<usize>,
}

impl<'rpc, T: Rpc + BuildConfigSimulator> EmulatorAccountStorage<'rpc, T> {
    pub async fn new(
        rpc: &'rpc T,
        program_id: Pubkey,
        chains: Option<Vec<ChainInfo>>,
        block_overrides: Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
        solana_overrides: Option<SolanaOverrides>,
        tx_chain_id: Option<u64>,
    ) -> Result<EmulatorAccountStorage<T>, NeonError> {
        trace!("backend::new");

        let block_number = match block_overrides.as_ref().and_then(|o| o.number) {
            None => rpc.get_slot().await?,
            Some(number) => number,
        };

        let block_timestamp = match block_overrides.as_ref().and_then(|o| o.time) {
            None => rpc.get_block_time(block_number).await?,
            Some(time) => time,
        };

        let chains = match chains {
            None => crate::commands::get_config::read_chains(rpc, program_id).await?,
            Some(chains) => chains,
        };

        let rent_account = rpc
            .get_account(&solana_sdk::sysvar::rent::id())
            .await?
            .ok_or(NeonError::AccountNotFound(solana_sdk::sysvar::rent::id()))?;

        let rent = bincode::deserialize::<Rent>(&rent_account.data)?;
        info!("Rent: {rent:?}");

        let accounts_cache = FrozenMap::new();
        if let Some(overrides) = solana_overrides {
            for (pubkey, account) in overrides {
                accounts_cache.insert(pubkey, Box::new(account));
            }
        }
        let storage = Self {
            accounts: FrozenMap::new(),
            call_stack: vec![],
            program_id,
            operator: FAKE_OPERATOR,
            chains,
            gas: 0,
            realloc_iterations: 0,
            execute_status: ExecuteStatus::default(),
            rpc,
            block_number,
            block_timestamp,
            timestamp_used: RefCell::new(false),
            state_overrides,
            rent,
            accounts_cache,
            used_accounts: FrozenMap::new(),
            return_data: None,
            logs: vec![],
            logs_stack: vec![],
        };

        let target_chain_id = tx_chain_id.unwrap_or_else(|| storage.default_chain_id());
        storage.apply_balance_overrides(target_chain_id).await?;

        Ok(storage)
    }

    pub async fn new_from_other(
        other: &Self,
        block_shift: u64,
        timestamp_shift: i64,
        tx_chain_id: Option<u64>,
    ) -> Result<EmulatorAccountStorage<'rpc, T>, NeonError> {
        let storage = Self {
            accounts: FrozenMap::new(),
            call_stack: vec![],
            program_id: other.program_id,
            operator: other.operator,
            chains: other.chains.clone(),
            gas: 0,
            realloc_iterations: 0,
            execute_status: ExecuteStatus::default(),
            rpc: other.rpc,
            block_number: other.block_number.saturating_add(block_shift),
            block_timestamp: other.block_timestamp.saturating_add(timestamp_shift),
            timestamp_used: RefCell::new(false),
            rent: Rent {
                lamports_per_byte_year: other.rent.lamports_per_byte_year,
                exemption_threshold: other.rent.exemption_threshold,
                burn_percent: other.rent.burn_percent,
            },
            state_overrides: other.state_overrides.clone(),
            accounts_cache: other.accounts_cache.clone(),
            used_accounts: other.used_accounts.clone(),
            return_data: None,
            logs: other.logs.clone(),
            logs_stack: other.logs_stack.clone(),
        };
        let target_chain_id = tx_chain_id.unwrap_or_else(|| storage.default_chain_id());
        storage.apply_balance_overrides(target_chain_id).await?;
        Ok(storage)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn with_accounts(
        rpc: &'rpc T,
        program_id: Pubkey,
        accounts: &[Pubkey],
        chains: Option<Vec<ChainInfo>>,
        block_overrides: Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
        solana_overrides: Option<SolanaOverrides>,
        tx_chain_id: Option<u64>,
    ) -> Result<EmulatorAccountStorage<'rpc, T>, NeonError> {
        let storage = Self::new(
            rpc,
            program_id,
            chains,
            block_overrides,
            state_overrides,
            solana_overrides,
            tx_chain_id,
        )
        .await?;

        storage.download_accounts(accounts).await?;

        Ok(storage)
    }
}

impl<'a, T: Rpc> EmulatorAccountStorage<'_, T> {
    async fn apply_balance_overrides(&self, target_chain_id: u64) -> NeonResult<()> {
        if let Some(state_overrides) = self.state_overrides.as_ref() {
            for (address, overrides) in state_overrides {
                if overrides.nonce.is_none() && overrides.balance.is_none() {
                    continue;
                }
                let mut balance_data = self
                    .get_balance_account(*address, target_chain_id)
                    .await?
                    .borrow_mut();
                let mut balance = self.get_or_create_ethereum_balance(
                    &mut balance_data,
                    *address,
                    target_chain_id,
                )?;
                if let Some(nonce) = overrides.nonce {
                    info!("apply nonce overrides {address} -> {nonce}");
                    balance.override_nonce_by(nonce);
                }
                if let Some(expected_balance) = overrides.balance {
                    info!("apply balance overrides {address} -> {expected_balance}");
                    balance.override_balance_by(expected_balance);
                }
            }
        }
        Ok(())
    }

    async fn download_accounts(&self, pubkeys: &[Pubkey]) -> Result<(), NeonError> {
        let accounts = self.rpc.get_multiple_accounts(pubkeys).await?;

        for (key, account) in pubkeys.iter().zip(accounts) {
            self.accounts_cache.insert(*key, Box::new(account));
        }

        Ok(())
    }

    pub async fn _get_deactivated_solana_features(
        &self,
    ) -> solana_client::client_error::Result<Vec<Pubkey>> {
        self.rpc.get_deactivated_solana_features().await
    }

    pub async fn _get_account_from_rpc(
        &self,
        pubkey: Pubkey,
    ) -> solana_client::client_error::Result<Option<&Account>> {
        if pubkey == FAKE_OPERATOR {
            return Ok(None);
        }

        if let Some(account) = self.accounts_cache.get(&pubkey) {
            return Ok(account.as_ref());
        }

        let response = self.rpc.get_account(&pubkey).await?;
        let account = self.accounts_cache.insert(pubkey, Box::new(response));
        Ok(account.as_ref())
    }

    pub async fn _get_multiple_accounts_from_rpc(
        &self,
        pubkeys: &[Pubkey],
    ) -> solana_client::client_error::Result<Vec<Option<&Account>>> {
        let mut accounts = vec![None; pubkeys.len()];

        let mut exists = vec![true; pubkeys.len()];
        let mut missing_keys = Vec::with_capacity(pubkeys.len());

        for (i, pubkey) in pubkeys.iter().enumerate() {
            if pubkey == &FAKE_OPERATOR {
                continue;
            }

            let Some(account) = self.accounts_cache.get(pubkey) else {
                exists[i] = false;
                missing_keys.push(*pubkey);
                continue;
            };

            accounts[i] = account.as_ref();
        }

        let mut response = self.rpc.get_multiple_accounts(&missing_keys).await?;

        let mut j = 0_usize;
        for i in 0..pubkeys.len() {
            if exists[i] {
                continue;
            }

            let pubkey = missing_keys[j];
            let account = response[j].take();
            let account = self.accounts_cache.insert(pubkey, Box::new(account));
            // ^ .insert() returns the reference to the account that was just inserted

            assert_eq!(pubkeys[i], pubkey);
            accounts[i] = account.as_ref();

            j += 1;
        }

        Ok(accounts)
    }

    fn mark_account(&self, pubkey: Pubkey, is_writable: bool) {
        let mut data = self._get_account_mark(pubkey);
        data.is_writable |= is_writable;
    }

    fn mark_legacy_account(
        &self,
        pubkey: Pubkey,
        is_writable: bool,
        lamports_after_upgrade: Option<u64>,
    ) {
        let mut data = self._get_account_mark(pubkey);
        data.is_writable |= is_writable;
        data.is_legacy = true;
        if lamports_after_upgrade.is_some() {
            data.lamports_after_upgrade = lamports_after_upgrade;
        }
    }

    fn _get_account_mark(&self, pubkey: Pubkey) -> RefMut<'_, SolanaAccount> {
        self.used_accounts
            .insert(
                pubkey,
                Box::new(RefCell::new(SolanaAccount {
                    pubkey,
                    is_writable: false,
                    is_legacy: false,
                    lamports_after_upgrade: None,
                })),
            )
            .borrow_mut()
    }

    fn _add_legacy_account(
        &self,
        info: &AccountInfo<'_>,
    ) -> NeonResult<(&RefCell<AccountData>, &RefCell<AccountData>)> {
        let legacy = LegacyEtherData::from_account(&self.program_id, info)?;

        let (balance_pubkey, _) = legacy
            .address
            .find_balance_address(&self.program_id, self.default_chain_id());
        let balance_data = self.add_empty_account(balance_pubkey);
        if (legacy.balance > 0) || (legacy.trx_count > 0) {
            let mut balance_data = balance_data.borrow_mut();
            let mut balance = self.create_ethereum_balance(
                &mut balance_data,
                legacy.address,
                self.default_chain_id(),
            )?;
            balance.mint(legacy.balance)?;
            balance.increment_nonce_by(legacy.trx_count)?;
            self.mark_legacy_account(balance_pubkey, true, Some(balance_data.lamports));
        } else {
            self.mark_legacy_account(balance_pubkey, false, Some(0));
        }

        let (contract_pubkey, _) = legacy.address.find_solana_address(&self.program_id);
        let contract_data = self.add_empty_account(contract_pubkey);
        if (legacy.code_size > 0) || (legacy.generation > 0) {
            let code = legacy.read_code(info);
            let storage = legacy.read_storage(info);

            let mut contract_data = contract_data.borrow_mut();
            let mut contract = self.create_ethereum_contract(
                &mut contract_data,
                legacy.address,
                self.default_chain_id(),
                legacy.generation,
                &code,
            )?;
            if !code.is_empty() {
                contract.set_storage_multiple_values(0, &storage);
            }
            self.mark_legacy_account(contract_pubkey, true, Some(contract_data.lamports));
        } else {
            // We have to mark account as writable, because we destroy the original legacy account
            self.mark_legacy_account(contract_pubkey, true, Some(0));
        }

        Ok((contract_data, balance_data))
    }

    async fn _get_contract_generation_limited(&self, address: Address) -> NeonResult<Option<u32>> {
        let extract_generation = |contract_data: &RefCell<AccountData>| -> NeonResult<Option<u32>> {
            let mut contract_data = contract_data.borrow_mut();
            if contract_data.is_empty() {
                Ok(None)
            } else {
                let contract = ContractAccount::from_account(
                    &self.program_id,
                    contract_data.into_account_info(),
                )?;
                if contract.code().len() > 0 {
                    Ok(Some(contract.generation()))
                } else {
                    Ok(None)
                }
            }
        };

        let (pubkey, _) = address.find_solana_address(&self.program_id);
        let contract_data = if let Some(contract_data) = self.accounts.get(&pubkey) {
            contract_data
        } else {
            let mut account = self._get_account_from_rpc(pubkey).await?.cloned();
            if let Some(account) = &mut account {
                let info = account_info(&pubkey, account);
                if *info.owner == self.program_id {
                    match evm_loader::account::tag(&self.program_id, &info)? {
                        evm_loader::account::TAG_ACCOUNT_CONTRACT => {
                            let data = AccountData::new_from_account(pubkey, account);
                            self.accounts.insert(pubkey, Box::new(RefCell::new(data)))
                        }
                        evm_loader::account::legacy::TAG_ACCOUNT_CONTRACT_DEPRECATED => self
                            ._add_legacy_account(&info)
                            .map(|(contract, _balance)| contract)?,
                        _ => {
                            unimplemented!();
                        }
                    }
                } else {
                    let account_data = AccountData::new_from_account(pubkey, account);
                    self.accounts
                        .insert(pubkey, Box::new(RefCell::new(account_data)))
                }
            } else {
                self.add_empty_account(pubkey)
            }
        };
        self.mark_legacy_account(pubkey, false, None);
        extract_generation(contract_data)
    }

    async fn _add_legacy_storage(
        &self,
        legacy_storage: &LegacyStorageData,
        info: &AccountInfo<'_>,
        pubkey: Pubkey,
    ) -> NeonResult<&RefCell<AccountData>> {
        let generation = self
            ._get_contract_generation_limited(legacy_storage.address)
            .await?;
        let storage_data = self.add_empty_account(pubkey);

        if Some(legacy_storage.generation) == generation {
            let cells = legacy_storage.read_cells(info);

            let mut storage_data = storage_data.borrow_mut();
            self.create_ethereum_storage(&mut storage_data)?;

            storage_data.expand(StorageCell::required_account_size(cells.len()));
            storage_data.lamports = self.rent.minimum_balance(storage_data.get_length());
            let mut storage =
                StorageCell::from_account(&self.program_id, storage_data.into_account_info())?;
            storage.cells_mut().copy_from_slice(&cells);
            self.mark_legacy_account(pubkey, true, Some(storage_data.lamports));
        } else {
            self.mark_legacy_account(pubkey, true, Some(0));
        }
        Ok(storage_data)
    }

    async fn add_account(
        &self,
        pubkey: Pubkey,
        account: &Account,
    ) -> NeonResult<&RefCell<AccountData>> {
        let mut account = account.clone();
        let info = account_info(&pubkey, &mut account);
        if *info.owner == self.program_id {
            let tag = evm_loader::account::tag(&self.program_id, &info)?;
            match tag {
                evm_loader::account::TAG_ACCOUNT_BALANCE
                | evm_loader::account::TAG_ACCOUNT_CONTRACT
                | evm_loader::account::TAG_STORAGE_CELL => {
                    // TODO: update header from previous revisions
                    let account_data = AccountData::new_from_account(pubkey, &account);
                    self.mark_account(pubkey, false);
                    Ok(self
                        .accounts
                        .insert(pubkey, Box::new(RefCell::new(account_data))))
                }
                evm_loader::account::legacy::TAG_ACCOUNT_CONTRACT_DEPRECATED => self
                    ._add_legacy_account(&info)
                    .map(|(contract, _balance)| contract),
                evm_loader::account::legacy::TAG_STORAGE_CELL_DEPRECATED => {
                    let legacy_storage = LegacyStorageData::from_account(&self.program_id, &info)?;
                    self._add_legacy_storage(&legacy_storage, &info, pubkey)
                        .await
                }
                _ => {
                    unimplemented!();
                }
            }
        } else {
            let account_data = AccountData::new_from_account(pubkey, &account);
            self.mark_account(pubkey, false);
            Ok(self
                .accounts
                .insert(pubkey, Box::new(RefCell::new(account_data))))
        }
    }

    fn add_empty_account(&self, pubkey: Pubkey) -> &RefCell<AccountData> {
        let account_data = AccountData::new(pubkey);
        self.mark_account(pubkey, false);
        self.accounts
            .insert(pubkey, Box::new(RefCell::new(account_data)))
    }

    async fn use_account(
        &self,
        pubkey: Pubkey,
        is_writable: bool,
    ) -> NeonResult<&RefCell<AccountData>> {
        if pubkey == self.operator() {
            return Err(EvmLoaderError::InvalidAccountForCall(pubkey).into());
        }

        self.mark_account(pubkey, is_writable);

        if let Some(account) = self.accounts.get(&pubkey) {
            return Ok(account);
        }

        let account = self._get_account_from_rpc(pubkey).await?;
        if let Some(account) = account {
            self.add_account(pubkey, account).await
        } else {
            Ok(self.add_empty_account(pubkey))
        }
    }

    async fn get_balance_account(
        &self,
        address: Address,
        chain_id: u64,
    ) -> NeonResult<&RefCell<AccountData>> {
        let (pubkey, _) = address.find_balance_address(self.program_id(), chain_id);

        if let Some(account) = self.accounts.get(&pubkey) {
            return Ok(account);
        }

        match self._get_account_from_rpc(pubkey).await? {
            Some(account) => self.add_account(pubkey, account).await,
            None => {
                if chain_id == self.default_chain_id() {
                    let (legacy_pubkey, _) = address.find_solana_address(self.program_id());
                    if self.accounts.get(&legacy_pubkey).is_some() {
                        // We already have information about contract account (empty or filled with data).
                        // So the balance should be updated, but it is missed. So return the empty account.
                        Ok(self.add_empty_account(pubkey))
                    } else {
                        // We didn't process contract account and we doesn't have any information about it.
                        // So we can try to process account which can be a legacy.
                        if let Some(legacy_account) =
                            self._get_account_from_rpc(legacy_pubkey).await?
                        {
                            self.add_account(legacy_pubkey, legacy_account).await?;
                            self.accounts
                                .get(&pubkey)
                                .map_or_else(|| Ok(self.add_empty_account(pubkey)), Ok)
                        } else {
                            self.add_empty_account(legacy_pubkey);
                            Ok(self.add_empty_account(pubkey))
                        }
                    }
                } else {
                    Ok(self.add_empty_account(pubkey))
                }
            }
        }
    }

    async fn get_contract_account(&self, address: Address) -> NeonResult<&RefCell<AccountData>> {
        let (pubkey, _) = address.find_solana_address(self.program_id());

        if let Some(account) = self.accounts.get(&pubkey) {
            return Ok(account);
        }

        match self._get_account_from_rpc(pubkey).await? {
            Some(account) => self.add_account(pubkey, account).await,
            None => Ok(self.add_empty_account(pubkey)),
        }
    }

    async fn get_storage_account(
        &self,
        address: Address,
        index: U256,
    ) -> NeonResult<&RefCell<AccountData>> {
        let (base, _) = address.find_solana_address(self.program_id());
        let cell_address = StorageCellAddress::new(self.program_id(), &base, &index);
        let cell_pubkey = *cell_address.pubkey();

        if let Some(account) = self.accounts.get(&cell_pubkey) {
            return Ok(account);
        }

        match self._get_account_from_rpc(cell_pubkey).await? {
            Some(account) => self.add_account(cell_pubkey, account).await,
            None => Ok(self.add_empty_account(cell_pubkey)),
        }
    }

    pub async fn ethereum_balance_map_or<F, R>(
        &self,
        address: Address,
        chain_id: u64,
        default: R,
        action: F,
    ) -> NeonResult<R>
    where
        F: FnOnce(&BalanceAccount) -> R,
    {
        let mut balance_data = self
            .get_balance_account(address, chain_id)
            .await?
            .borrow_mut();
        if balance_data.is_empty() {
            Ok(default)
        } else {
            let account_info = balance_data.into_account_info();
            let balance = BalanceAccount::from_account(self.program_id(), account_info)?;
            Ok(action(&balance))
        }
    }

    pub async fn ethereum_contract_map_or<F, R>(
        &self,
        address: Address,
        default: R,
        action: F,
    ) -> NeonResult<R>
    where
        F: FnOnce(&ContractAccount) -> R,
    {
        let mut contract_data = self.get_contract_account(address).await?.borrow_mut();
        if contract_data.is_empty() {
            Ok(default)
        } else {
            let account_info = contract_data.into_account_info();
            let contract = ContractAccount::from_account(self.program_id(), account_info)?;
            Ok(action(&contract))
        }
    }

    pub async fn ethereum_storage_map_or<F, R>(
        &self,
        address: Address,
        index: U256,
        default: R,
        action: F,
    ) -> NeonResult<R>
    where
        F: FnOnce(&StorageCell) -> R,
    {
        let mut storage_data = self.get_storage_account(address, index).await?.borrow_mut();
        if storage_data.is_empty() {
            Ok(default)
        } else {
            let account_info = storage_data.into_account_info();
            let storage = StorageCell::from_account(self.program_id(), account_info)?;
            Ok(action(&storage))
        }
    }

    fn create_ethereum_balance(
        &'a self,
        account_data: &'a mut RefMut<AccountData>,
        address: Address,
        chain_id: u64,
    ) -> evm_loader::error::Result<BalanceAccount> {
        let required_len = BalanceAccount::required_account_size();
        account_data.assign(self.program_id)?;
        account_data.expand(required_len);
        account_data.lamports = self.rent.minimum_balance(account_data.get_length());

        BalanceAccount::initialize(
            account_data.into_account_info(),
            &self.program_id,
            address,
            chain_id,
        )
    }

    fn get_or_create_ethereum_balance(
        &'a self,
        account_data: &'a mut RefMut<AccountData>,
        address: Address,
        chain_id: u64,
    ) -> evm_loader::error::Result<BalanceAccount> {
        if account_data.is_empty() {
            self.create_ethereum_balance(account_data, address, chain_id)
        } else {
            BalanceAccount::from_account(&self.program_id, account_data.into_account_info())
        }
    }

    fn create_ethereum_contract(
        &'a self,
        account_data: &'a mut RefMut<AccountData>,
        address: Address,
        chain_id: u64,
        generation: u32,
        code: &[u8],
    ) -> evm_loader::error::Result<ContractAccount> {
        self.mark_account(account_data.pubkey, true);
        let required_len = ContractAccount::required_account_size(code);
        account_data.assign(self.program_id)?;
        account_data.expand(required_len);
        account_data.lamports = self.rent.minimum_balance(account_data.get_length());

        ContractAccount::initialize(
            account_data.into_account_info(),
            &self.program_id,
            address,
            chain_id,
            generation,
            code,
        )
    }

    fn create_ethereum_storage(
        &'a self,
        account_data: &'a mut RefMut<AccountData>,
    ) -> evm_loader::error::Result<StorageCell> {
        self.mark_account(account_data.pubkey, true);
        account_data.assign(self.program_id)?;
        account_data.expand(StorageCell::required_account_size(0));
        account_data.lamports = self.rent.minimum_balance(account_data.get_length());

        StorageCell::initialize(account_data.into_account_info(), &self.program_id)
    }

    fn get_or_create_ethereum_storage(
        &'a self,
        account_data: &'a mut RefMut<AccountData>,
    ) -> evm_loader::error::Result<StorageCell> {
        if account_data.is_empty() {
            self.create_ethereum_storage(account_data)
        } else {
            StorageCell::from_account(&self.program_id, account_data.into_account_info())
        }
    }

    async fn mint(
        &mut self,
        address: Address,
        chain_id: u64,
        value: U256,
    ) -> evm_loader::error::Result<()> {
        info!("mint {address}:{chain_id} {value}");
        let mut balance_data = self
            .get_balance_account(address, chain_id)
            .await
            .map_err(map_neon_error)?
            .borrow_mut();

        let mut balance =
            self.get_or_create_ethereum_balance(&mut balance_data, address, chain_id)?;
        balance.mint(value)?;
        balance.update_lamports(&self.rent);
        self.mark_account(balance_data.pubkey, true);

        Ok(())
    }

    pub fn used_accounts(&self) -> Vec<SolanaAccount> {
        self.used_accounts
            .clone()
            .into_map()
            .values()
            .map(|v| v.borrow().clone())
            .collect::<Vec<_>>()
    }

    pub fn accounts_get(&self, pubkey: &Pubkey) -> Option<Ref<AccountData>> {
        self.accounts.get(pubkey).map(RefCell::borrow)
    }

    pub fn get_upgrade_rent(&self) -> evm_loader::error::Result<u64> {
        let mut lamports_collected = 0u64;
        let mut lamports_spend = 0u64;
        for (_, used_account) in self.used_accounts.clone().into_tuple_vec() {
            let used_account = used_account.borrow();
            let pubkey = used_account.pubkey;
            if let Some(lamports_after_upgrade) = used_account.lamports_after_upgrade {
                let orig_lamports = self
                    .accounts_cache
                    .get(&used_account.pubkey)
                    .unwrap_or(&None)
                    .as_ref()
                    .map_or(0, |v| v.lamports);
                debug!("Upgrade rent: {pubkey} {orig_lamports} -> {lamports_after_upgrade}");
                if lamports_after_upgrade > orig_lamports {
                    lamports_spend += lamports_after_upgrade - orig_lamports;
                } else {
                    lamports_collected += orig_lamports - lamports_after_upgrade;
                }
            }
        }
        debug!(
            "Upgrade rent: {lamports_spend} - {lamports_collected} = {}",
            lamports_spend.saturating_sub(lamports_collected)
        );
        Ok(lamports_spend.saturating_sub(lamports_collected))
    }

    pub fn get_regular_rent(&self) -> evm_loader::error::Result<u64> {
        let accounts = self.accounts.clone();
        let mut old_lamports_sum = 0u64;
        let mut new_lamports_sum = 0u64;
        for (pubkey, account) in &accounts.into_map() {
            if *pubkey == system_program::ID {
                continue;
            }

            let (original_lamports, original_size) =
                self.accounts_cache.get(pubkey).map_or((0, 0), |v| {
                    v.as_ref().map_or((0, 0), |v| (v.lamports, v.data.len()))
                });

            let lamports_after_upgrade = self
                .used_accounts
                .get(pubkey)
                .and_then(|v| v.borrow().lamports_after_upgrade);

            let new_acc = account.borrow();
            let new_lamports = new_acc.lamports;
            let new_size = new_acc.get_length();

            if new_acc.is_busy() && new_lamports < self.rent.minimum_balance(new_acc.get_length()) {
                info!("Account {pubkey} is not rent exempt");
                return Err(ProgramError::AccountNotRentExempt.into());
            }

            let old_lamports = lamports_after_upgrade.unwrap_or(original_lamports);
            old_lamports_sum += old_lamports;
            new_lamports_sum += new_lamports;

            #[allow(clippy::cast_possible_wrap)]
            let diff = new_lamports as i64 - old_lamports as i64;
            debug!("Changes in rent: {pubkey} {old_lamports} -> {new_lamports} = {diff} | {original_size} -> {new_size} {lamports_after_upgrade:?}");
        }
        let changes_in_rent = new_lamports_sum.saturating_sub(old_lamports_sum);
        info!("Changes in rent: {changes_in_rent} = {new_lamports_sum} - {old_lamports_sum}");
        Ok(changes_in_rent)
    }

    pub fn get_changes_in_rent(&self) -> evm_loader::error::Result<u64> {
        Ok(self.get_upgrade_rent()? + self.get_regular_rent()?)
    }

    pub fn is_timestamp_used(&self) -> bool {
        *self.timestamp_used.borrow()
    }

    pub fn logs(&self) -> Vec<Log> {
        self.logs.clone()
    }
}

impl<T: Rpc> LogCollector for EmulatorAccountStorage<'_, T> {
    fn collect_log<const N: usize>(
        &mut self,
        address: &[u8; 20],
        topics: [[u8; 32]; N],
        data: &[u8],
    ) {
        self.logs.push(Log {
            address: address.into(),
            topics: topics.iter().map(Into::into).collect(),
            data: data.into(),
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            transaction_log_index: None,
            log_type: None,
            removed: None,
        });
    }
}

#[async_trait(?Send)]
impl<T: Rpc> AccountStorage for EmulatorAccountStorage<'_, T> {
    fn program_id(&self) -> &Pubkey {
        debug!("program_id");
        &self.program_id
    }

    fn operator(&self) -> Pubkey {
        info!("operator");
        self.operator
    }

    fn block_number(&self) -> U256 {
        info!("block_number");
        self.block_number.into()
    }

    fn block_timestamp(&self) -> U256 {
        info!("block_timestamp");
        *self.timestamp_used.borrow_mut() = true;
        self.block_timestamp.try_into().unwrap()
    }

    fn rent(&self) -> &Rent {
        &self.rent
    }

    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
        info!("return_data");
        self.return_data
            .as_ref()
            .map(|data| (data.program_id, data.data.clone()))
    }

    fn set_return_data(&mut self, data: &[u8]) {
        info!("set_return_data");
        self.return_data = Some(TransactionReturnData {
            program_id: self.program_id,
            data: data.to_vec(),
        });
    }

    async fn block_hash(&self, slot: u64) -> [u8; 32] {
        info!("block_hash {slot}");

        if let Ok(account) = self.use_account(slot_hashes::ID, false).await {
            let account_data = account.borrow();
            let data = account_data.data();
            if !data.is_empty() {
                return find_slot_hash(slot, data);
            }
        }
        panic!("Error querying account {} from Solana", slot_hashes::ID)
    }

    async fn nonce(&self, address: Address, chain_id: u64) -> u64 {
        info!("nonce {address}  {chain_id}");

        self.ethereum_balance_map_or(
            address,
            chain_id,
            u64::default(),
            |account: &BalanceAccount| account.nonce(),
        )
        .await
        .unwrap()
    }

    async fn balance(&self, address: Address, chain_id: u64) -> U256 {
        info!("balance {address} {chain_id}");

        self.ethereum_balance_map_or(
            address,
            chain_id,
            U256::default(),
            |account: &BalanceAccount| account.balance(),
        )
        .await
        .unwrap()
    }

    fn is_valid_chain_id(&self, chain_id: u64) -> bool {
        for chain in &self.chains {
            if chain.id == chain_id {
                return true;
            }
        }

        false
    }

    fn chain_id_to_token(&self, chain_id: u64) -> Pubkey {
        for chain in &self.chains {
            if chain.id == chain_id {
                return chain.token;
            }
        }

        unreachable!();
    }

    fn default_chain_id(&self) -> u64 {
        for chain in &self.chains {
            if chain.name == "neon" {
                return chain.id;
            }
        }

        unreachable!();
    }

    async fn contract_chain_id(&self, address: Address) -> evm_loader::error::Result<u64> {
        let default_value = Err(EvmLoaderError::Custom(std::format!(
            "Account {address} - invalid tag"
        )));

        self.ethereum_contract_map_or(address, default_value, |a| Ok(a.chain_id()))
            .await
            .unwrap()
    }

    fn contract_pubkey(&self, address: Address) -> (Pubkey, u8) {
        address.find_solana_address(self.program_id())
    }

    fn balance_pubkey(&self, address: Address, chain_id: u64) -> (Pubkey, u8) {
        address.find_balance_address(self.program_id(), chain_id)
    }

    fn storage_cell_pubkey(&self, address: Address, index: U256) -> Pubkey {
        let base = self.contract_pubkey(address).0;
        if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u64) {
            base
        } else {
            let address = StorageCellAddress::new(self.program_id(), &base, &index);
            *address.pubkey()
        }
    }

    async fn code_size(&self, address: Address) -> usize {
        info!("code_size {address}");

        self.code(address).await.len()
    }

    async fn code(&self, address: Address) -> evm_loader::evm::Buffer {
        use evm_loader::evm::Buffer;

        info!("code {address}");

        // TODO: move to reading data from Solana node
        // let code_override = self.account_override(address, |a| a.code.clone());
        // if let Some(code_override) = code_override {
        //     return Buffer::from_vec(code_override.0);
        // }

        let code = self
            .ethereum_contract_map_or(address, Vec::default(), |c| c.code().to_vec())
            .await
            .unwrap();

        Buffer::from_vec(code)
    }

    async fn storage(&self, address: Address, index: U256) -> [u8; 32] {
        // TODO: move to reading data from Solana node
        // let storage_override = self.account_override(address, |a| a.storage(index));
        // if let Some(storage_override) = storage_override {
        //     return storage_override;
        // }

        let value = if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u64) {
            let index: usize = index.as_usize();
            self.ethereum_contract_map_or(address, [0_u8; 32], |c| c.storage_value(index))
                .await
                .unwrap()
        } else {
            let subindex = (index & 0xFF).as_u8();
            let index = index & !U256::new(0xFF);

            self.ethereum_storage_map_or(address, index, <[u8; 32]>::default(), |cell| {
                cell.get(subindex)
            })
            .await
            .unwrap()
        };

        info!("storage {address} -> {index} = {}", hex::encode(value));

        value
    }

    async fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        info!("clone_solana_account {}", address);

        if *address == self.operator() {
            let mut account = fake_operator();
            let info = account_info(address, &mut account);
            OwnedAccountInfo::from_account_info(self.program_id(), &info)
        } else {
            let account = self
                .use_account(*address, false)
                .await
                .expect("Error querying account from Solana");

            let mut account_data = account.borrow_mut();
            let info = account_data.into_account_info();
            OwnedAccountInfo::from_account_info(self.program_id(), &info)
        }
    }

    async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R,
    {
        let account = self
            .use_account(*address, false)
            .await
            .expect("Error querying account from Solana");

        let mut account_data = account.borrow_mut();
        let info = account_data.into_account_info();
        action(&info)
    }
}

#[allow(clippy::needless_pass_by_value)]
fn map_neon_error(e: NeonError) -> EvmLoaderError {
    EvmLoaderError::Custom(e.to_string())
}

#[async_trait(?Send)]
impl<T: Rpc> SyncedAccountStorage for EmulatorAccountStorage<'_, T> {
    async fn set_code(
        &mut self,
        address: Address,
        chain_id: u64,
        code: Vec<u8>,
    ) -> evm_loader::error::Result<()> {
        info!("set_code {address} -> {} bytes", code.len());
        {
            let mut account_data = self
                .get_contract_account(address)
                .await
                .map_err(map_neon_error)?
                .borrow_mut();
            let pubkey = account_data.pubkey;

            if account_data.is_empty() {
                self.create_ethereum_contract(&mut account_data, address, chain_id, 0, &code)?;
            } else {
                let contract = ContractAccount::from_account(
                    self.program_id(),
                    account_data.into_account_info(),
                )?;
                if contract.code().len() > 0 {
                    return Err(EvmLoaderError::AccountAlreadyInitialized(
                        account_data.pubkey,
                    ));
                }
                let new_account_data = RefCell::new(AccountData::new(pubkey));
                {
                    let mut new_account = new_account_data.borrow_mut();
                    let mut new_contract = self.create_ethereum_contract(
                        &mut new_account,
                        address,
                        chain_id,
                        contract.generation(),
                        &code,
                    )?;
                    let storage = *contract.storage();
                    new_contract.set_storage_multiple_values(0, &storage);
                }
                *account_data = new_account_data.replace_with(|_| AccountData::new(pubkey));
            }
        }

        let realloc = ContractAccount::required_account_size(&code)
            / solana_sdk::entrypoint::MAX_PERMITTED_DATA_INCREASE;
        self.realloc_iterations = self.realloc_iterations.max(realloc as u64);

        Ok(())
    }

    async fn set_storage(
        &mut self,
        address: Address,
        index: U256,
        value: [u8; 32],
    ) -> evm_loader::error::Result<()> {
        info!("set_storage {address} -> {index} = {}", hex::encode(value));
        const STATIC_STORAGE_LIMIT: U256 = U256::new(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u128);

        if index < STATIC_STORAGE_LIMIT {
            let mut contract_data = self
                .get_contract_account(address)
                .await
                .map_err(map_neon_error)?
                .borrow_mut();

            let mut contract = if contract_data.is_empty() {
                self.create_ethereum_contract(&mut contract_data, address, 0, 0, &[])?
            } else {
                ContractAccount::from_account(self.program_id(), contract_data.into_account_info())?
            };
            contract.set_storage_value(index.as_usize(), &value);
            contract.update_lamports(&self.rent);
            self.mark_account(contract_data.pubkey, true);
        } else {
            let subindex = (index & 0xFF).as_u8();
            let index = index & !U256::new(0xFF);

            let mut storage_data = self
                .get_storage_account(address, index)
                .await
                .map_err(map_neon_error)?
                .borrow_mut();

            let mut storage = self.get_or_create_ethereum_storage(&mut storage_data)?;
            storage.update(subindex, &value)?;
            storage.update_lamports(&self.rent);
            self.mark_account(storage_data.pubkey, true);
        }

        Ok(())
    }

    async fn increment_nonce(
        &mut self,
        address: Address,
        chain_id: u64,
    ) -> evm_loader::error::Result<()> {
        info!("nonce increment {address} {chain_id}");
        let mut balance_data = self
            .get_balance_account(address, chain_id)
            .await
            .map_err(map_neon_error)?
            .borrow_mut();
        let mut balance =
            self.get_or_create_ethereum_balance(&mut balance_data, address, chain_id)?;
        balance.increment_nonce()?;
        balance.update_lamports(&self.rent);
        self.mark_account(balance_data.pubkey, true);

        Ok(())
    }

    async fn transfer(
        &mut self,
        from_address: Address,
        to_address: Address,
        chain_id: u64,
        value: U256,
    ) -> evm_loader::error::Result<()> {
        self.burn(from_address, chain_id, value).await?;
        self.mint(to_address, chain_id, value).await?;

        Ok(())
    }

    async fn burn(
        &mut self,
        address: Address,
        chain_id: u64,
        value: U256,
    ) -> evm_loader::error::Result<()> {
        info!("burn {address} {chain_id} {value}");
        let mut balance_data = self
            .get_balance_account(address, chain_id)
            .await
            .map_err(map_neon_error)?
            .borrow_mut();
        self.mark_account(balance_data.pubkey, true);

        let mut balance =
            self.get_or_create_ethereum_balance(&mut balance_data, address, chain_id)?;
        balance.burn(value)?;
        balance.update_lamports(&self.rent);

        Ok(())
    }

    async fn execute_external_instruction(
        &mut self,
        instruction: Instruction,
        seeds: Vec<Vec<Vec<u8>>>,
        _fee: u64,
        emulated_internally: bool,
    ) -> evm_loader::error::Result<()> {
        use solana_sdk::{message::Message, signature::Signer, transaction::Transaction};

        info!("execute_external_instruction: {instruction:?}");
        info!("Operator: {}", self.operator);
        self.execute_status.external_solana_call |= !emulated_internally;

        let mut solana_simulator = SolanaSimulator::new(self)
            .await
            .map_err(|e| EvmLoaderError::Custom(e.to_string()))?;

        solana_simulator.set_sysvar(&Clock {
            slot: self.block_number,
            epoch_start_timestamp: self.block_timestamp,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: self.block_timestamp,
        });

        let signers = seeds
            .iter()
            .map(|s| {
                let seed = s.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let signer = Pubkey::create_program_address(&seed, &self.program_id)?;
                Ok(signer)
            })
            .collect::<Result<HashSet<_>, PubkeyError>>()?;
        info!("Signers: {signers:?}");

        let mut accounts = Vec::new();
        accounts.push(instruction.program_id);
        self.mark_account(instruction.program_id, false);

        for meta in &instruction.accounts {
            if meta.pubkey != self.operator {
                self.use_account(meta.pubkey, meta.is_writable)
                    .await
                    .map_err(map_neon_error)?;
                if meta.is_signer && !signers.contains(&meta.pubkey) {
                    return Err(ProgramError::MissingRequiredSignature.into());
                }
            }
            accounts.push(meta.pubkey);
        }

        solana_simulator
            .sync_accounts(self, &accounts)
            .await
            .map_err(|e| EvmLoaderError::Custom(e.to_string()))?;

        let trx = Transaction::new_unsigned(Message::new_with_blockhash(
            &[instruction.clone()],
            Some(&solana_simulator.payer().pubkey()),
            &solana_simulator.blockhash(),
        ));

        let result = solana_simulator
            .simulate_legacy_transaction(trx)
            .map_err(|e| EvmLoaderError::Custom(e.to_string()))?;

        if let Err(error) = result.result {
            return Err(EvmLoaderError::ExternalCallFailed(
                instruction.program_id,
                error.to_string(),
            ));
        }

        if let Some(return_data) = result.return_data {
            self.return_data = Some(return_data);
        }

        for meta in &instruction.accounts {
            if meta.pubkey == self.operator {
                continue;
            }
            let account = result
                .post_simulation_accounts
                .iter()
                .find(|(pubkey, _)| *pubkey == meta.pubkey)
                .map(|(_, account)| account)
                .ok_or_else(|| {
                    EvmLoaderError::Custom(format!("Account {} not found", meta.pubkey))
                })?;

            let mut account_data = self
                .accounts
                .get(&meta.pubkey)
                .ok_or_else(|| {
                    EvmLoaderError::Custom(format!("Account data {} not found", meta.pubkey))
                })?
                .borrow_mut();

            *account_data = AccountData::new_from_account(meta.pubkey, account);
        }

        Ok(())
    }

    fn snapshot(&mut self) {
        info!("snapshot");
        self.call_stack.push(self.accounts.clone());
        self.logs_stack.push(self.logs.len());
    }

    fn revert_snapshot(&mut self) {
        info!("revert_snapshot");
        self.accounts = self.call_stack.pop().expect("No snapshots to revert");

        if self.execute_status.external_solana_call {
            self.execute_status.reverts_after_solana_calls = true;
        } else {
            self.execute_status.reverts_before_solana_calls = true;
        }

        let logs_stack_len = self
            .logs_stack
            .pop()
            .expect("Fatal Error: Inconsistent EVM Logs Stack");

        self.logs.truncate(logs_stack_len);

        if self.logs_stack.is_empty() {
            // sanity check
            assert!(self.logs.is_empty());
        }
    }

    fn commit_snapshot(&mut self) {
        self.call_stack.pop().expect("No snapshots to commit");
        self.logs_stack
            .pop()
            .expect("Fatal Error: Inconsistent EVM Logs Stack");
    }
}

#[must_use]
pub const fn fake_operator() -> Account {
    Account {
        lamports: 100 * 1_000_000_000,
        data: vec![],
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
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

#[cfg(test)]
#[path = "./account_storage_tests.rs"]
mod account_storage_tests;
