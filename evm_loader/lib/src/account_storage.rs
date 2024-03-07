use async_trait::async_trait;
use evm_loader::account::legacy::{
    LegacyEtherData, LegacyStorageData, TAG_ACCOUNT_CONTRACT_DEPRECATED,
    TAG_STORAGE_CELL_DEPRECATED,
};
use evm_loader::account::{TAG_ACCOUNT_CONTRACT, TAG_STORAGE_CELL};
use evm_loader::account_storage::find_slot_hash;
use evm_loader::types::{Address, Vector};
use solana_sdk::rent::Rent;
use solana_sdk::system_program;
use solana_sdk::sysvar::slot_hashes;
use std::collections::HashSet;
use std::{cell::RefCell, collections::HashMap, convert::TryInto, rc::Rc};

use crate::NeonResult;
use crate::{rpc::Rpc, NeonError};
use ethnum::U256;
use evm_loader::{
    account::{BalanceAccount, ContractAccount, StorageCell, StorageCellAddress},
    account_storage::AccountStorage,
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    executor::{Action, OwnedAccountInfo},
};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use solana_client::client_error;
use solana_sdk::{account::Account, account_info::AccountInfo, pubkey, pubkey::Pubkey};

use crate::commands::get_config::{BuildConfigSimulator, ChainInfo};
use crate::tracing::{AccountOverride, AccountOverrides, BlockOverrides};
use serde_with::{serde_as, DisplayFromStr};

const FAKE_OPERATOR: Pubkey = pubkey!("neonoperator1111111111111111111111111111111");

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaAccount {
    #[serde_as(as = "DisplayFromStr")]
    pubkey: Pubkey,
    is_writable: bool,
    is_legacy: bool,
    #[serde(skip)]
    data: Option<Account>,
}

#[allow(clippy::module_name_repetitions)]
pub struct EmulatorAccountStorage<'rpc, T: Rpc> {
    pub accounts: RefCell<HashMap<Pubkey, SolanaAccount>>,
    pub gas: u64,
    rpc: &'rpc T,
    program_id: Pubkey,
    chains: Vec<ChainInfo>,
    block_number: u64,
    block_timestamp: i64,
    rent: Rent,
    state_overrides: Option<AccountOverrides>,
}

impl<'rpc, T: Rpc + BuildConfigSimulator> EmulatorAccountStorage<'rpc, T> {
    pub async fn new(
        rpc: &'rpc T,
        program_id: Pubkey,
        chains: Option<Vec<ChainInfo>>,
        block_overrides: Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
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
            .value
            .ok_or(NeonError::AccountNotFound(solana_sdk::sysvar::rent::id()))?;
        let rent = bincode::deserialize::<Rent>(&rent_account.data)?;
        info!("Rent: {rent:?}");

        Ok(Self {
            accounts: RefCell::new(HashMap::new()),
            program_id,
            chains,
            gas: 0,
            rpc,
            block_number,
            block_timestamp,
            state_overrides,
            rent,
        })
    }

    pub async fn with_accounts(
        rpc: &'rpc T,
        program_id: Pubkey,
        accounts: &[Pubkey],
        chains: Option<Vec<ChainInfo>>,
        block_overrides: Option<BlockOverrides>,
        state_overrides: Option<AccountOverrides>,
    ) -> Result<EmulatorAccountStorage<'rpc, T>, NeonError> {
        let storage = Self::new(rpc, program_id, chains, block_overrides, state_overrides).await?;

        storage.download_accounts(accounts).await?;

        Ok(storage)
    }
}

impl<T: Rpc> EmulatorAccountStorage<'_, T> {
    async fn download_accounts(&self, pubkeys: &[Pubkey]) -> Result<(), NeonError> {
        let accounts = self.rpc.get_multiple_accounts(pubkeys).await?;

        let mut cache = self.accounts.borrow_mut();

        for (key, account) in pubkeys.iter().zip(accounts) {
            let account = SolanaAccount {
                pubkey: *key,
                is_writable: false,
                is_legacy: false,
                data: account.clone(),
            };

            cache.insert(*key, account);
        }

        Ok(())
    }

    pub async fn use_account(
        &self,
        pubkey: Pubkey,
        is_writable: bool,
    ) -> client_error::Result<Option<Account>> {
        if pubkey == FAKE_OPERATOR {
            return Ok(None);
        }

        if let Some(account) = self.accounts.borrow_mut().get_mut(&pubkey) {
            account.is_writable |= is_writable;
            return Ok(account.data.clone());
        }

        let response = self.rpc.get_account(&pubkey).await?;
        let account = response.value;

        self.accounts.borrow_mut().insert(
            pubkey,
            SolanaAccount {
                pubkey,
                is_writable,
                is_legacy: false,
                data: account.clone(),
            },
        );

        Ok(account)
    }

    pub async fn use_balance_account(
        &self,
        address: Address,
        chain_id: u64,
        is_writable: bool,
    ) -> NeonResult<(Pubkey, Option<Account>, Option<Account>)> {
        let (pubkey, _) = address.find_balance_address(self.program_id(), chain_id);
        let account = self.use_account(pubkey, is_writable).await?;

        let legacy_account = if account.is_none() && (chain_id == self.default_chain_id()) {
            let (legacy_pubkey, _) = address.find_solana_address(self.program_id());
            self.use_account(legacy_pubkey, is_writable).await?
        } else {
            None
        };

        Ok((pubkey, account, legacy_account))
    }

    pub async fn use_contract_account(
        &self,
        address: Address,
        is_writable: bool,
    ) -> NeonResult<(Pubkey, Option<Account>)> {
        let (pubkey, _) = address.find_solana_address(self.program_id());
        let account = self.use_account(pubkey, is_writable).await?;

        Ok((pubkey, account))
    }

    pub async fn use_storage_cell(
        &self,
        address: Address,
        index: U256,
        is_writable: bool,
    ) -> NeonResult<(Pubkey, Option<Account>)> {
        let (base, _) = address.find_solana_address(self.program_id());
        let cell_address = StorageCellAddress::new(self.program_id(), &base, &index);

        let account = self
            .use_account(*cell_address.pubkey(), is_writable)
            .await?;

        Ok((*cell_address.pubkey(), account))
    }

    pub async fn apply_actions(&mut self, actions: Vector<Action>) -> Result<(), NeonError> {
        info!("apply_actions");

        let mut new_balance_accounts = HashSet::new();

        for action in actions {
            #[allow(clippy::match_same_arms)]
            match action {
                Action::Transfer {
                    source,
                    target,
                    chain_id,
                    value,
                } => {
                    info!("neon transfer {value} from {source} to {target}");

                    self.use_balance_account(source, chain_id, true).await?;

                    let (key, target, legacy) =
                        self.use_balance_account(target, chain_id, true).await?;
                    if target.is_none() && legacy.is_none() {
                        new_balance_accounts.insert(key);
                    }
                }
                Action::Burn {
                    source,
                    value,
                    chain_id,
                } => {
                    info!("neon withdraw {value} from {source}");

                    self.use_balance_account(source, chain_id, true).await?;
                }
                Action::EvmSetStorage {
                    address,
                    index,
                    value,
                } => {
                    info!("set storage {address} -> {index} = {}", hex::encode(value));

                    if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u64) {
                        self.use_contract_account(address, true).await?;
                    } else {
                        let index = index & !U256::new(0xFF);
                        let (_, account) = self.use_storage_cell(address, index, true).await?;

                        let cell_size = StorageCell::required_account_size(1);
                        let empty_size = StorageCell::required_account_size(0);

                        let gas = if account.is_none() {
                            self.rent.minimum_balance(cell_size)
                        } else {
                            let existing_value = self.storage(address, index).await;
                            if existing_value == [0_u8; 32] {
                                self.rent
                                    .minimum_balance(cell_size)
                                    .saturating_sub(self.rent.minimum_balance(empty_size))
                            } else {
                                0
                            }
                        };

                        self.gas = self.gas.saturating_add(gas);
                    }
                }
                Action::EvmIncrementNonce { address, chain_id } => {
                    info!("nonce increment {address}");

                    let (key, account, legacy) =
                        self.use_balance_account(address, chain_id, true).await?;
                    if account.is_none() && legacy.is_none() {
                        new_balance_accounts.insert(key);
                    }
                }
                Action::EvmSetCode {
                    address,
                    code,
                    chain_id: _,
                } => {
                    info!("set code {address} -> {} bytes", code.len());
                    self.use_contract_account(address, true).await?;

                    let space = ContractAccount::required_account_size(&code);
                    self.gas = self.gas.saturating_add(self.rent.minimum_balance(space));
                }
                Action::EvmSelfDestruct { address } => {
                    info!("selfdestruct {address}");
                }
                Action::ExternalInstruction {
                    program_id,
                    accounts,
                    fee,
                    ..
                } => {
                    info!("external call {program_id}");

                    self.use_account(program_id, false).await?;

                    for account in accounts {
                        self.use_account(account.pubkey, account.is_writable)
                            .await?;
                    }

                    self.gas = self.gas.saturating_add(fee);
                }
            }
        }

        self.gas = self.gas.saturating_add(
            self.rent
                .minimum_balance(BalanceAccount::required_account_size())
                .saturating_mul(new_balance_accounts.len() as u64),
        );

        Ok(())
    }

    pub async fn mark_legacy_accounts(&mut self) -> Result<(), NeonError> {
        let mut accounts = self.accounts.borrow_mut();
        let mut additional_balances = Vec::new();

        for (key, account) in accounts.iter_mut() {
            let Some(account_data) = account.data.as_mut() else {
                continue;
            };

            let info = account_info(key, account_data);
            if info.owner != self.program_id() {
                continue;
            }

            let Ok(tag) = evm_loader::account::tag(self.program_id(), &info) else {
                continue;
            };

            if tag == TAG_STORAGE_CELL_DEPRECATED {
                account.is_writable = true;
                account.is_legacy = true;
            }

            if tag == TAG_ACCOUNT_CONTRACT_DEPRECATED {
                account.is_writable = true;
                account.is_legacy = true;

                let legacy_data = LegacyEtherData::from_account(self.program_id(), &info)?;
                additional_balances.push(legacy_data.address);

                if (legacy_data.code_size > 0) || (legacy_data.generation > 0) {
                    // This is a contract, we need additional gas for conversion
                    let lamports = self
                        .rent
                        .minimum_balance(BalanceAccount::required_account_size());
                    self.gas = self.gas.saturating_add(lamports);
                }
            }

            if !account.is_writable {
                continue;
            }

            let required_header_realloc = match tag {
                TAG_ACCOUNT_CONTRACT => {
                    let contract = ContractAccount::from_account(self.program_id(), info)?;
                    contract.required_header_realloc()
                }
                TAG_STORAGE_CELL => {
                    let cell = StorageCell::from_account(self.program_id(), info)?;
                    cell.required_header_realloc()
                }
                _ => 0,
            };

            let header_realloc_lamports = self
                .rent
                .minimum_balance(required_header_realloc)
                .saturating_sub(self.rent.minimum_balance(0));

            self.gas = self.gas.saturating_add(header_realloc_lamports);
        }

        for a in additional_balances {
            let (pubkey, _) = a.find_balance_address(self.program_id(), self.default_chain_id());
            let account = SolanaAccount {
                pubkey,
                is_writable: true,
                is_legacy: false,
                data: None,
            };

            accounts.insert(pubkey, account);
        }

        Ok(())
    }

    pub async fn ethereum_balance_map_or<F, L, R>(
        &self,
        address: Address,
        chain_id: u64,
        default: R,
        legacy_action: L,
        action: F,
    ) -> R
    where
        L: FnOnce(LegacyEtherData) -> R,
        F: FnOnce(BalanceAccount) -> R,
    {
        let (pubkey, mut account, mut legacy) = self
            .use_balance_account(address, chain_id, false)
            .await
            .unwrap();

        if let Some(account_data) = &mut account {
            let info = account_info(&pubkey, account_data);
            if let Ok(a) = BalanceAccount::from_account(self.program_id(), info) {
                return action(a);
            }
        }

        if chain_id != self.default_chain_id() {
            return default;
        }

        if let Some(legacy_data) = &mut legacy {
            let info = account_info(&pubkey, legacy_data);
            if let Ok(a) = LegacyEtherData::from_account(self.program_id(), &info) {
                return legacy_action(a);
            }
        }

        default
    }

    pub async fn ethereum_contract_map_or<F, L, R>(
        &self,
        address: Address,
        default: R,
        legacy_action: L,
        action: F,
    ) -> R
    where
        L: FnOnce(LegacyEtherData, &AccountInfo) -> R,
        F: FnOnce(ContractAccount) -> R,
    {
        let (pubkey, mut account) = self.use_contract_account(address, false).await.unwrap();

        let Some(account_data) = &mut account else {
            return default;
        };

        if system_program::check_id(&account_data.owner) {
            return default;
        }

        let info = account_info(&pubkey, account_data);
        let Ok(tag) = evm_loader::account::tag(self.program_id(), &info) else {
            return default;
        };

        match tag {
            TAG_ACCOUNT_CONTRACT => {
                let contract = ContractAccount::from_account(self.program_id(), info).unwrap();
                action(contract)
            }
            TAG_ACCOUNT_CONTRACT_DEPRECATED => {
                let legacy_data = LegacyEtherData::from_account(self.program_id(), &info).unwrap();
                legacy_action(legacy_data, &info)
            }
            _ => default,
        }
    }

    pub async fn ethereum_storage_map_or<F, L, R>(
        &self,
        address: Address,
        index: U256,
        default: R,
        legacy_action: L,
        action: F,
    ) -> R
    where
        L: FnOnce(LegacyStorageData, &AccountInfo) -> R,
        F: FnOnce(StorageCell) -> R,
    {
        let (pubkey, mut account) = self.use_storage_cell(address, index, false).await.unwrap();

        let Some(account_data) = &mut account else {
            return default;
        };

        if system_program::check_id(&account_data.owner) {
            return default;
        }

        let info = account_info(&pubkey, account_data);
        let Ok(tag) = evm_loader::account::tag(self.program_id(), &info) else {
            return default;
        };

        match tag {
            TAG_STORAGE_CELL => {
                let contract = StorageCell::from_account(self.program_id(), info).unwrap();
                action(contract)
            }
            TAG_STORAGE_CELL_DEPRECATED => {
                let legacy_data =
                    LegacyStorageData::from_account(self.program_id(), &info).unwrap();
                legacy_action(legacy_data, &info)
            }
            _ => default,
        }
    }

    fn account_override<F, R>(&self, address: Address, f: F) -> Option<R>
    where
        F: FnOnce(&AccountOverride) -> Option<R>,
    {
        self.state_overrides
            .as_ref()
            .and_then(|a| a.get(&address))
            .and_then(f)
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
        FAKE_OPERATOR
    }

    fn block_number(&self) -> U256 {
        info!("block_number");
        self.block_number.into()
    }

    fn block_timestamp(&self) -> U256 {
        info!("block_timestamp");
        self.block_timestamp.try_into().unwrap()
    }

    fn rent(&self) -> &Rent {
        &self.rent
    }

    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
        info!("return_data");
        // TODO: implement return_data() method with SyncedAccountStorage implementation
        unimplemented!();
    }

    async fn block_hash(&self, slot: u64) -> [u8; 32] {
        info!("block_hash {slot}");

        if let Ok(Some(slot_hashes_account)) = self.use_account(slot_hashes::ID, false).await {
            let slot_hashes_data = slot_hashes_account.data.as_slice();
            find_slot_hash(slot, slot_hashes_data)
        } else {
            panic!("Error querying account {} from Solana", slot_hashes::ID)
        }
    }

    async fn nonce(&self, address: Address, chain_id: u64) -> u64 {
        info!("nonce {address}  {chain_id}");

        let nonce_override = self.account_override(address, |a| a.nonce);
        if let Some(nonce_override) = nonce_override {
            return nonce_override;
        }

        self.ethereum_balance_map_or(
            address,
            chain_id,
            u64::default(),
            |legacy| legacy.trx_count,
            |account| account.nonce(),
        )
        .await
    }

    async fn balance(&self, address: Address, chain_id: u64) -> U256 {
        info!("balance {address} {chain_id}");

        let balance_override = self.account_override(address, |a| a.balance);
        if let Some(balance_override) = balance_override {
            return balance_override;
        }

        self.ethereum_balance_map_or(
            address,
            chain_id,
            U256::default(),
            |legacy| legacy.balance,
            |account| account.balance(),
        )
        .await
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
        use evm_loader::error::Error;

        let default_value = Err(Error::Custom(std::format!(
            "Account {address} - invalid tag"
        )));

        self.ethereum_contract_map_or(
            address,
            default_value,
            |_legacy, _| Ok(self.default_chain_id()),
            |a| Ok(a.chain_id()),
        )
        .await
    }

    fn contract_pubkey(&self, address: Address) -> (Pubkey, u8) {
        address.find_solana_address(self.program_id())
    }

    async fn code_size(&self, address: Address) -> usize {
        info!("code_size {address}");

        self.code(address).await.len()
    }

    async fn code(&self, address: Address) -> evm_loader::evm::Buffer {
        use evm_loader::evm::Buffer;

        info!("code {address}");

        let code_override = self.account_override(address, |a| a.code.clone());
        if let Some(code_override) = code_override {
            return Buffer::from_vec(code_override.0);
        }

        let code = self
            .ethereum_contract_map_or(
                address,
                Vec::default(),
                |legacy, info| legacy.read_code(info),
                |c| c.code().to_vec(),
            )
            .await;

        Buffer::from_vec(code)
    }

    async fn storage(&self, address: Address, index: U256) -> [u8; 32] {
        let storage_override = self.account_override(address, |a| a.storage(index));
        if let Some(storage_override) = storage_override {
            return storage_override;
        }

        let value = if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u64) {
            let index: usize = index.as_usize();
            self.ethereum_contract_map_or(
                address,
                [0_u8; 32],
                |legacy, info| legacy.read_storage(info)[index],
                |c| c.storage_value(index),
            )
            .await
        } else {
            let subindex = (index & 0xFF).as_u8();
            let index = index & !U256::new(0xFF);

            self.ethereum_storage_map_or(
                address,
                index,
                <[u8; 32]>::default(),
                |legacy, info| legacy.read_value(subindex, info),
                |cell| cell.get(subindex),
            )
            .await
        };

        info!("storage {address} -> {index} = {}", hex::encode(value));

        value
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
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }
        } else {
            let mut account = self
                .use_account(*address, false)
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
        let mut account = self
            .use_account(*address, false)
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
