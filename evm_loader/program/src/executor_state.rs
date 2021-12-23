#![allow(missing_docs, clippy::missing_panics_doc, clippy::missing_errors_doc)]

/// Todo: document

use core::mem;
use std::{
    boxed::Box,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    vec::Vec
};

use evm::{ExitError, H160, H256, Transfer, U256, Valids};
use evm::backend::{Apply, Log};
use evm::gasometer::Gasometer;
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

use crate::solana_backend::AccountStorage;
use crate::utils::keccak256_h256;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExecutorAccount {
    pub nonce: U256,
    #[serde(with = "serde_bytes")]
    pub code: Option<Vec<u8>>,
    #[serde(with = "serde_bytes")]
    pub valids: Option<Vec<u8>>,
    pub reset: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ExecutorMetadata {
    gasometer: Gasometer,
    is_static: bool,
    depth: Option<usize>,
    block_number: U256,
    block_timestamp: U256,
}

impl ExecutorMetadata {
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new<B: AccountStorage>(gas_limit: u64, backend: &B) -> Self {
        Self {
            gasometer: Gasometer::new(gas_limit),
            is_static: false,
            depth: None,
            block_number: backend.block_number(),
            block_timestamp: backend.block_timestamp()
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn swallow_commit(&mut self, other: Self) -> Result<(), ExitError> {
	    self.gasometer.record_stipend(other.gasometer.gas())?;
        self.gasometer
            .record_refund(other.gasometer.refunded_gas())?;

    	// The following fragment deleted in the mainstream code:
        // if let Some(runtime) = self.runtime.borrow_mut().as_ref() {
        //     let return_value = other.borrow().runtime().unwrap().machine().return_value();
        //     runtime.set_return_data(return_value);
        // }

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn swallow_revert(&mut self, other: Self) -> Result<(), ExitError> {
        self.gasometer.record_stipend(other.gasometer.gas())?;

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value, clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
        Ok(())
    }

    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn spit_child(&self, gas_limit: u64, is_static: bool) -> Self {
        Self {
            gasometer: Gasometer::new(gas_limit),
            is_static: is_static || self.is_static,
            depth: match self.depth {
                None => Some(0),
                Some(n) => Some(n + 1),
            },
            block_number: self.block_number,
            block_timestamp: self.block_timestamp,
        }
    }

    #[must_use]
    pub const fn gasometer(&self) -> &Gasometer {
        &self.gasometer
    }

    pub fn gasometer_mut(&mut self) -> &mut Gasometer {
        &mut self.gasometer
    }

    #[allow(dead_code)]
    #[must_use]
    pub const fn is_static(&self) -> bool {
        self.is_static
    }

    #[must_use]
    pub const fn depth(&self) -> Option<usize> {
        self.depth
    }

    #[must_use]
    pub const fn block_number(&self) -> &U256 {
        &self.block_number
    }

    #[must_use]
    pub const fn block_timestamp(&self) -> &U256 {
        &self.block_timestamp
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SplTransfer {
    pub source: H160,
    pub target: H160,
    pub contract: H160,
    pub mint: Pubkey,
    pub source_token: Pubkey,
    pub target_token: Pubkey,
    pub value: u64
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SplApprove {
    pub owner: H160,
    pub spender: Pubkey,
    pub contract: H160,
    pub mint: Pubkey,
    pub value: u64
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ERC20Approve {
    pub owner: H160,
    pub spender: H160,
    pub contract: H160,
    pub mint: Pubkey,
    pub value: U256
}

#[derive(Serialize, Deserialize)]
pub struct ExecutorSubstate {
    metadata: ExecutorMetadata,
    parent: Option<Box<ExecutorSubstate>>,
    logs: Vec<Log>,
    transfers: Vec<Transfer>,
    accounts: BTreeMap<H160, ExecutorAccount>,
    balances: RefCell<BTreeMap<H160, U256>>,
    storages: BTreeMap<(H160, U256), U256>,
    spl_balances: RefCell<BTreeMap<Pubkey, u64>>,
    spl_decimals: RefCell<BTreeMap<Pubkey, u8>>,
    spl_supply: RefCell<BTreeMap<Pubkey, u64>>,
    spl_transfers: Vec<SplTransfer>,
    spl_approves: Vec<SplApprove>,
    erc20_allowances: BTreeMap<(H160, H160, H160, Pubkey), U256>,
    deletes: BTreeSet<H160>,
}

pub type ApplyState = (Vec::<Apply<BTreeMap<U256, U256>>>, Vec<Log>, Vec<Transfer>, Vec<SplTransfer>, Vec<SplApprove>, Vec<ERC20Approve>);

impl ExecutorSubstate {
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new<B: AccountStorage>(gas_limit: u64, backend: &B) -> Self {
        Self {
            metadata: ExecutorMetadata::new(gas_limit, backend),
            parent: None,
            logs: Vec::new(),
            transfers: Vec::new(),
            accounts: BTreeMap::new(),
            balances: RefCell::new(BTreeMap::new()),
            storages: BTreeMap::new(),
            spl_balances: RefCell::new(BTreeMap::new()),
            spl_decimals: RefCell::new(BTreeMap::new()),
            spl_supply: RefCell::new(BTreeMap::new()),
            spl_transfers: Vec::new(),
            spl_approves: Vec::new(),
            erc20_allowances: BTreeMap::new(),
            deletes: BTreeSet::new(),
        }
    }

    #[must_use]
    pub const fn metadata(&self) -> &ExecutorMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut ExecutorMetadata {
        &mut self.metadata
    }

    /// Deconstruct the executor, return state to be applied. Panic if the
    /// executor is not in the top-level substate.
    #[must_use]
    pub fn deconstruct<B: AccountStorage>(
        mut self,
        backend: &B,
    ) -> ApplyState {
        assert!(self.parent.is_none());

        let mut applies = Vec::<Apply<BTreeMap<U256, U256>>>::new();

        let mut addresses = BTreeSet::new();

        for address in self.accounts.keys() {
            addresses.insert(*address);
        }

        for (address, _) in self.storages.keys() {
            addresses.insert(*address);
        }

        for address in addresses {
            if self.deletes.contains(&address) {
                continue;
            }

            let mut storage = BTreeMap::new();
            for ((oa, ok), ov) in &self.storages {
                if *oa == address {
                    storage.insert(*ok, *ov);
                }
            }

            let apply = {
                let account = self.accounts.remove(&address).unwrap_or_else(
                    || ExecutorAccount {
                        nonce: backend.nonce(&address),
                        code: None,
                        valids: None,
                        reset: false,
                    }
                );

                Apply::Modify {
                    address,
                    nonce: account.nonce,
                    code_and_valids: account.code.zip(account.valids),
                    storage,
                    reset_storage: account.reset,
                }
            };

            applies.push(apply);
        }

        for address in self.deletes {
            applies.push(Apply::Delete { address });
        }

        let mut erc20_approves = Vec::with_capacity(self.erc20_allowances.len());
        for ((owner, spender, contract, mint), value) in self.erc20_allowances {
            let approve = ERC20Approve { owner, spender, contract, mint, value };
            erc20_approves.push(approve);
        }

        (applies, self.logs, self.transfers, self.spl_transfers, self.spl_approves, erc20_approves)
    }

    pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
        let mut entering = Self {
            metadata: self.metadata.spit_child(gas_limit, is_static),
            parent: None,
            logs: Vec::new(),
            transfers: Vec::new(),
            accounts: BTreeMap::new(),
            balances: RefCell::new(BTreeMap::new()),
            storages: BTreeMap::new(),
            spl_balances: RefCell::new(BTreeMap::new()),
            spl_decimals: RefCell::new(BTreeMap::new()),
            spl_supply: RefCell::new(BTreeMap::new()),
            spl_transfers: Vec::new(),
            spl_approves: Vec::new(),
            erc20_allowances: BTreeMap::new(),
            deletes: BTreeSet::new(),
        };
        mem::swap(&mut entering, self);

        self.parent = Some(Box::new(entering));
    }

    pub fn exit_commit(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot commit on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_commit(exited.metadata)?;
        self.logs.append(&mut exited.logs);
        self.balances.borrow_mut().append(&mut exited.balances.borrow_mut());
        self.transfers.append(&mut exited.transfers);

        self.spl_balances.borrow_mut().append(&mut exited.spl_balances.borrow_mut());
        self.spl_decimals.borrow_mut().append(&mut exited.spl_decimals.borrow_mut());
        self.spl_supply.borrow_mut().append(&mut exited.spl_supply.borrow_mut());
        self.spl_transfers.append(&mut exited.spl_transfers);
        self.spl_approves.append(&mut exited.spl_approves);

        self.erc20_allowances.append(&mut exited.erc20_allowances);

        let mut resets = BTreeSet::new();
        for (address, account) in &exited.accounts {
            if account.reset {
                resets.insert(*address);
            }
        }
        let mut reset_keys = BTreeSet::new();
        for (address, key) in self.storages.keys() {
            if resets.contains(address) {
                reset_keys.insert((*address, *key));
            }
        }
        for (address, key) in reset_keys {
            self.storages.remove(&(address, key));
        }

        resets = BTreeSet::new();
        for (address, account) in &self.accounts {
            if account.reset {
                resets.insert(*address);
            }
        }
        self.accounts.append(&mut exited.accounts);
        self.storages.append(&mut exited.storages);
        self.deletes.append(&mut exited.deletes);

        for address in &resets {
            if self.accounts.contains_key(address){
                self.accounts.get_mut(address).unwrap().reset = true;
            }
        }

        Ok(())
    }

    pub fn exit_revert(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_revert(exited.metadata)?;

        Ok(())
    }

    pub fn exit_discard(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_discard(exited.metadata)?;

        Ok(())
    }

    fn known_account(&self, address: H160) -> Option<&ExecutorAccount> {
        match self.accounts.get(&address) {
            Some(account) => Some(account),
            None => self.parent.as_ref().and_then(|parent| parent.known_account(address))
        }
    }

    #[must_use]
    pub fn known_nonce(&self, address: H160) -> Option<U256> {
        self.known_account(address).map(|acc| acc.nonce)
    }

    #[must_use]
    pub fn known_code(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.code.clone())
    }

    #[must_use]
    pub fn known_valids(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.valids.clone())
    }

    #[must_use]
    pub fn known_empty(&self, address: H160) -> Option<bool> {
        if let Some(balance) = self.known_balance(&address) {
            if balance != U256::zero() {
                return Some(false);
            }
        } else {
            return None;
        }

        if let Some(account) = self.known_account(address) {
            if account.nonce != U256::zero() {
                return Some(false);
            }

            if let Some(code) = &account.code {
                return Some(code.is_empty());
            }
        }

        None
    }

    #[must_use]
    pub fn known_storage(&self, address: H160, key: U256) -> Option<U256> {
        if let Some(value) = self.storages.get(&(address, key)) {
            return Some(*value);
        }

        if let Some(account) = self.accounts.get(&address) {
            if account.reset {
                return Some(U256::zero());
            }
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.known_storage(address, key);
        }

        None
    }

    #[must_use]
    pub fn known_original_storage(&self, address: H160, key: U256) -> Option<U256> {
        if let Some(account) = self.accounts.get(&address) {
            if account.reset {
                return Some(U256::zero());
            }
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.known_original_storage(address, key);
        }

        None
    }

    #[must_use]
    pub fn deleted(&self, address: H160) -> bool {
        if self.deletes.contains(&address) {
            return true;
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.deleted(address);
        }

        false
    }

    #[must_use]
    fn account_mut<B: AccountStorage>(&mut self, address: H160, backend: &B) -> &mut ExecutorAccount {
        #[allow(clippy::map_entry)]
        if !self.accounts.contains_key(&address) {
            let account = self.known_account(address).cloned().map_or_else(
                || ExecutorAccount {
                    nonce: backend.nonce(&address),
                    code: None,
                    valids: None,
                    reset: false,
                },
                |mut v| {
                    v.reset = false;
                    v
                },
            );
            self.accounts.insert(address, account);
        }

        self.accounts
            .get_mut(&address)
            .expect("New account was just inserted")
    }

    pub fn inc_nonce<B: AccountStorage>(&mut self, address: H160, backend: &B) {
        let account = self.account_mut(address, backend);

        let (nonce, _overflow) = account.nonce.overflowing_add(U256::one());
        account.nonce = nonce;
    }

    pub fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        self.storages.insert((address, key), value);
    }

    pub fn reset_storage<B: AccountStorage>(&mut self, address: H160, backend: &B) {
        let mut removing = Vec::new();

        for (oa, ok) in self.storages.keys() {
            if *oa == address {
                removing.push(*ok);
            }
        }

        for ok in removing {
            self.storages.remove(&(address, ok));
        }

        self.account_mut(address, backend).reset = true;
    }

    pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.logs.push(Log {
            address,
            topics,
            data,
        });
    }

    pub fn set_deleted(&mut self, address: H160) {
        self.deletes.insert(address);
    }

    pub fn set_code<B: AccountStorage>(&mut self, address: H160, code: Vec<u8>, backend: &B) {
        self.account_mut(address, backend).valids = Some(Valids::compute(&code));
        self.account_mut(address, backend).code = Some(code);
    }

    #[must_use]
    pub fn known_balance(&self, address: &H160) -> Option<U256> {
        let balances = self.balances.borrow();

        match balances.get(address) {
            Some(balance) => Some(*balance),
            None => self.parent.as_ref().and_then(|parent| parent.known_balance(address))
        }
    }

    #[must_use]
    pub fn balance<B: AccountStorage>(&self, address: &H160, backend: &B) -> U256 {
        let value = self.known_balance(address);

        value.map_or_else(
            || {
                let balance = backend.balance(address);
                self.balances.borrow_mut().insert(*address, balance);

                balance
            },
            |value| value
        )
    }

    pub fn transfer<B: AccountStorage>(
        &mut self,
        transfer: &Transfer,
        backend: &B,
    ) -> Result<(), ExitError> {
        let new_source_balance = {
            let balance = self.balance(&transfer.source, backend);
            balance.checked_sub(transfer.value).ok_or(ExitError::OutOfFund)?
        };

        let new_target_balance = {
            let balance = self.balance(&transfer.target, backend);
            balance.checked_add(transfer.value).ok_or(ExitError::InvalidRange)?
        };

        let mut balances = self.balances.borrow_mut();
        balances.insert(transfer.source, new_source_balance);
        balances.insert(transfer.target, new_target_balance);

        self.transfers.push(*transfer);

        Ok(())
    }

    pub fn reset_balance(&self, address: H160) {
        let mut balances = self.balances.borrow_mut();
        balances.insert(address, U256::zero());
    }

    pub fn touch<B: AccountStorage>(&mut self, address: H160, backend: &B) {
        let _unused = self.account_mut(address, backend);
    }

    fn known_spl_balance(&self, address: &Pubkey) -> Option<u64> {
        let spl_balances = self.spl_balances.borrow();

        match spl_balances.get(address) {
            Some(balance) => Some(*balance),
            None => self.parent.as_ref().and_then(|parent| parent.known_spl_balance(address))
        }
    }

    #[must_use]
    pub fn spl_balance<B: AccountStorage>(&self, address: &Pubkey, backend: &B) -> u64 {
        let value = self.known_spl_balance(address);

        value.map_or_else(
            || {
                let balance = backend.get_spl_token_balance(address);
                self.spl_balances.borrow_mut().insert(*address, balance);

                balance
            },
            |value| value
        )
    }

    fn spl_transfer<B: AccountStorage>(&mut self, transfer: SplTransfer, backend: &B) -> Result<(), ExitError> {
        debug_print!("spl_transfer: {:?}", transfer);

        let new_source_balance = {
            let balance = self.spl_balance(&transfer.source_token, backend);
            balance.checked_sub(transfer.value).ok_or(ExitError::OutOfFund)?
        };

        let new_target_balance = {
            let balance = self.spl_balance(&transfer.target_token, backend);
            balance.checked_add(transfer.value).ok_or(ExitError::InvalidRange)?
        };

        let mut spl_balances = self.spl_balances.borrow_mut();
        spl_balances.insert(transfer.source_token, new_source_balance);
        spl_balances.insert(transfer.target_token, new_target_balance);

        self.spl_transfers.push(transfer);

        Ok(())
    }

    fn spl_approve(&mut self, approve: SplApprove) {
        self.spl_approves.push(approve);
    }

    fn known_spl_decimals(&self, address: &Pubkey) -> Option<u8> {
        let spl_decimals = self.spl_decimals.borrow();

        match spl_decimals.get(address) {
            Some(decimals) => Some(*decimals),
            None => self.parent.as_ref().and_then(|parent| parent.known_spl_decimals(address))
        }
    }

    #[must_use]
    pub fn spl_decimals<B: AccountStorage>(&self, address: &Pubkey, backend: &B) -> u8 {
        let value = self.known_spl_decimals(address);

        value.map_or_else(
            || {
                let decimals = backend.get_spl_token_decimals(address);
                self.spl_decimals.borrow_mut().insert(*address, decimals);

                decimals
            },
            |value| value
        )
    }

    fn known_spl_supply(&self, address: &Pubkey) -> Option<u64> {
        let spl_supply = self.spl_supply.borrow();

        match spl_supply.get(address) {
            Some(decimals) => Some(*decimals),
            None => self.parent.as_ref().and_then(|parent| parent.known_spl_supply(address))
        }
    }

    #[must_use]
    pub fn spl_supply<B: AccountStorage>(&self, address: &Pubkey, backend: &B) -> u64 {
        let value = self.known_spl_supply(address);

        value.map_or_else(
            || {
                let supply = backend.get_spl_token_supply(address);
                self.spl_supply.borrow_mut().insert(*address, supply);

                supply
            },
            |value| value
        )
    }

    fn known_erc20_allowance(&self, owner: H160, spender: H160, contract: H160, mint: Pubkey) -> Option<&U256> {
        match self.erc20_allowances.get(&(owner, spender, contract, mint)) {
            Some(allowance) => Some(allowance),
            None => self.parent.as_ref().and_then(|parent| parent.known_erc20_allowance(owner, spender, contract, mint))
        }
    }

    #[must_use]
    pub fn erc20_allowance<B: AccountStorage>(&self, owner: H160, spender: H160, contract: H160, mint: Pubkey, backend: &B) -> U256 {
        self.known_erc20_allowance(owner, spender, contract, mint)
            .copied()
            .unwrap_or_else(|| backend.get_erc20_allowance(&owner, &spender, &contract, &mint))
    }

    #[must_use]
    pub fn erc20_allowance_mut<B: AccountStorage>(&mut self, owner: H160, spender: H160, contract: H160, mint: Pubkey, backend: &B) -> &mut U256 {
        let key = (owner, spender, contract, mint);

        #[allow(clippy::map_entry)]
        if !self.erc20_allowances.contains_key(&key) {
            let allowance = self.erc20_allowance(owner, spender, contract, mint, backend);
            self.erc20_allowances.insert(key, allowance);
        }

        self.erc20_allowances
            .get_mut(&key)
            .expect("New allowance was just inserted")
    }

    fn erc20_approve(&mut self, approve: &ERC20Approve) {
        let key = (approve.owner, approve.spender, approve.contract, approve.mint);
        self.erc20_allowances.insert(key, approve.value);
    }
}

pub struct ExecutorState<'a, B: AccountStorage> {
    backend: &'a B,
    substate: Box<ExecutorSubstate>,
}

impl<'a, B: AccountStorage> ExecutorState<'a, B> {
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn gas_price(&self) -> U256 {
        // TODO correct gas price
        U256::zero()
    }

    #[must_use]
    pub fn origin(&self) -> H160 {
        self.backend.origin()
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn block_hash(&self, _number: U256) -> H256 {
        H256::default()
    }

    #[must_use]
    pub fn block_number(&self) -> U256 {
        *self.substate.metadata().block_number()
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn block_coinbase(&self) -> H160 {
        H160::default()
    }

    #[must_use]
    pub fn block_timestamp(&self) -> U256 {
        *self.substate.metadata().block_timestamp()
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn block_gas_limit(&self) -> U256 {
        U256::from(u64::MAX)
    }

    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn chain_id(&self) -> U256 {
        crate::config::chain_id()
    }

    #[must_use]
    pub fn exists(&self, address: H160) -> bool {
        self.substate.known_account(address).is_some() || self.backend.exists(&address)
    }

    #[must_use]
    pub fn nonce(&self, address: H160) -> U256 {
        self.substate
            .known_nonce(address)
            .unwrap_or_else(|| self.backend.nonce(&address))
    }

    #[must_use]
    pub fn balance(&self, address: H160) -> U256 {
        self.substate.balance(&address, self.backend)
    }

    #[must_use]
    pub fn code(&self, address: H160) -> Vec<u8> {
        self.substate
            .known_code(address)
            .unwrap_or_else(|| self.backend.code(&address))
    }

    #[must_use]
    pub fn code_hash(&self, address: H160) -> H256 {
        self.substate.known_code(address)
            .map_or_else(|| self.backend.code_hash(&address), |code| keccak256_h256(&code))
    }

    #[must_use]
    pub fn code_size(&self, address: H160) -> usize {
         self.substate.known_code(address)
            .map_or_else(|| self.backend.code_size(&address), |code| code.len())
    }

    #[must_use]
    pub fn valids(&self, address: H160) -> Vec<u8> {
        self.substate
            .known_valids(address)
            .unwrap_or_else(|| self.backend.valids(&address))
    }

    #[must_use]
    pub fn storage(&self, address: H160, key: U256) -> U256 {
        self.substate
            .known_storage(address, key)
            .unwrap_or_else(|| self.backend.storage(&address, &key))
    }

    #[must_use]
    pub fn metadata(&self) -> &ExecutorMetadata {
        self.substate.metadata()
    }

    #[must_use]
    pub fn metadata_mut(&mut self) -> &mut ExecutorMetadata {
        self.substate.metadata_mut()
    }

    pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
        self.substate.enter(gas_limit, is_static);
    }

    pub fn exit_commit(&mut self) -> Result<(), ExitError> {
        self.substate.exit_commit()
    }

    pub fn exit_revert(&mut self) -> Result<(), ExitError> {
        self.substate.exit_revert()
    }

    pub fn exit_discard(&mut self) -> Result<(), ExitError> {
        self.substate.exit_discard()
    }

    #[must_use]
    pub fn is_empty(&self, address: H160) -> bool {
        if let Some(known_empty) = self.substate.known_empty(address) {
            return known_empty;
        }

        self.backend.balance(&address) == U256::zero()
            && self.backend.nonce(&address) == U256::zero()
            && self.backend.code_size(&address) == 0
    }

    #[must_use]
    pub fn deleted(&self, address: H160) -> bool {
        self.substate.deleted(address)
    }

    pub fn inc_nonce(&mut self, address: H160) {
        self.substate.inc_nonce(address, self.backend);
    }

    pub fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        self.substate.set_storage(address, key, value);
    }

    pub fn reset_storage(&mut self, address: H160) {
        self.substate.reset_storage(address, self.backend);
    }

    #[must_use]
    pub fn original_storage(&self, address: H160, key: U256) -> Option<U256> {
        if let Some(value) = self.substate.known_original_storage(address, key) {
            return Some(value);
        }

        Some(self.backend.storage(&address, &key)) // todo backend.original_storage
    }

    pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.substate.log(address, topics, data);
    }

    pub fn set_deleted(&mut self, address: H160) {
        self.substate.set_deleted(address);
    }

    pub fn set_code(&mut self, address: H160, code: Vec<u8>) {
        self.substate.set_code(address, code, self.backend);
    }

    pub fn transfer(&mut self, transfer: &Transfer) -> Result<(), ExitError> {
        debug_print!("executor transfer from={} to={} value={}", transfer.source, transfer.target, transfer.value);
        if transfer.value.is_zero() {
            return Ok(())
        }

        self.substate.transfer(transfer, self.backend)
    }

    pub fn reset_balance(&mut self, address: H160) {
        self.substate.reset_balance(address);
    }

    pub fn touch(&mut self, address: H160) {
        self.substate.touch(address, self.backend);
    }

    #[must_use]
    pub fn erc20_decimals(&self, mint: Pubkey) -> u8
    {
        self.substate.spl_decimals(&mint, self.backend)
    }

    #[must_use]
    pub fn erc20_total_supply(&self, mint: Pubkey) -> U256
    {
        let supply = self.substate.spl_supply(&mint, self.backend);
        U256::from(supply)
    }

    /// Returns the account balance of another account with `address`.
    /// Returns zero if the account is not yet known.
    #[must_use]
    pub fn erc20_balance_of(&self, mint: Pubkey, context: &evm::Context, address: H160) -> U256
    {
        let (token_account, _) = self.backend.get_erc20_token_address(&address, &context.address, &mint);

        let balance = self.substate.spl_balance(&token_account, self.backend);
        U256::from(balance)
    }

    fn erc20_emit_transfer_event(&mut self, contract: H160, source: H160, target: H160, value: u64) {
        // event Transfer(address indexed from, address indexed to, uint256 value);

        let topics = vec![
            H256::from_str("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").unwrap(),
            H256::from(source),
            H256::from(target)
        ];

        let mut data = vec![0_u8; 32];
        U256::from(value).into_big_endian_fast(&mut data);

        self.log(contract, topics, data);
    }

    #[must_use]
    fn erc20_transfer_impl(&mut self, mint: Pubkey, contract: H160, source: H160, target: H160, value: U256) -> bool
    {
        if value > U256::from(u64::MAX) {
            return false;
        }
        let value = value.as_u64();

        let (source_token, _) = self.backend.get_erc20_token_address(&source, &contract, &mint);
        let (target_token, _) = self.backend.get_erc20_token_address(&target, &contract, &mint);

        let transfer = SplTransfer { source, target, contract, mint, source_token, target_token, value };
        if self.substate.spl_transfer(transfer, self.backend).is_err() {
            return false;
        }

        self.erc20_emit_transfer_event(contract, source, target, value);

        true
    }

    #[must_use]
    pub fn erc20_transfer(&mut self, mint: Pubkey, context: &evm::Context, target: H160, value: U256) -> bool
    {
        self.erc20_transfer_impl(mint, context.address, context.caller, target, value)
    }

    #[must_use]
    pub fn erc20_transfer_from(&mut self, mint: Pubkey, context: &evm::Context, source: H160, target: H160, value: U256) -> bool
    {
        let contract = context.address;

        {
            let allowance = self.substate.erc20_allowance_mut(source, context.caller, contract, mint, self.backend);
            if *allowance < value {
                return false;
            }
            *allowance -= value;
        }

        self.erc20_transfer_impl(mint, contract, source, target, value)
    }

    fn erc20_emit_approval_event(&mut self, contract: H160, owner: H160, spender: H160, value: U256) {
        // event Approval(address indexed owner, address indexed spender, uint256 value);

        let topics = vec![
            H256::from_str("8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925").unwrap(),
            H256::from(owner),
            H256::from(spender)
        ];

        let mut data = vec![0_u8; 32];
        value.into_big_endian_fast(&mut data);

        self.log(contract, topics, data);
    }

    pub fn erc20_approve(&mut self, mint: Pubkey, context: &evm::Context, spender: H160, value: U256)
    {
        let contract = context.address;
        let owner = context.caller;

        let approve = ERC20Approve { owner, spender, contract, mint, value };
        self.substate.erc20_approve(&approve);

        self.erc20_emit_approval_event(context.address, owner, spender, value);
    }

    #[must_use]
    pub fn erc20_allowance(&mut self, mint: Pubkey, context: &evm::Context, owner: H160, spender: H160) -> U256
    {
        let contract = context.address;

        self.substate.erc20_allowance(owner, spender, contract, mint, self.backend)
    }

    fn erc20_emit_approval_solana_event(&mut self, contract: H160, owner: H160, spender: Pubkey, value: u64) {
        // event ApprovalSolana(address indexed owner, bytes32 indexed spender, uint64 value);

        let topics = vec![
            H256::from_str("f2d0a01e4c49f3439199c8f8950e366e85c4d1bd845552f6da1009b3bb2c1a70").unwrap(),
            H256::from(owner),
            H256::from(spender.to_bytes())
        ];

        let mut data = vec![0_u8; 32];
        U256::from(value).into_big_endian_fast(&mut data);

        self.log(contract, topics, data);
    }

    pub fn erc20_approve_solana(&mut self, mint: Pubkey, context: &evm::Context, spender: Pubkey, value: u64)
    {
        let contract = context.address;
        let owner = context.caller;

        let approve = SplApprove { owner, spender, contract, mint, value };
        self.substate.spl_approve(approve);

        self.erc20_emit_approval_solana_event(context.address, owner, spender, value);
    }

    #[must_use]
    pub fn query_solana_account_owner(&self, address: Pubkey) -> Option<Pubkey> {
        let (found, owner) = self.backend.apply_to_solana_account(
            &address,
            || (false, Pubkey::default()),
            |info| (true, *info.owner),
        );
        if found { Some(owner) } else { None }
    }

    #[must_use]
    pub fn query_solana_account_length(&self, address: Pubkey) -> Option<usize> {
        let (found, length) = self.backend.apply_to_solana_account(
            &address,
            || (false, usize::default()),
            |info| (true, info.data.borrow().len()),
        );
        if found { Some(length) } else { None }
    }

    #[must_use]
    pub fn query_solana_account_lamports(&self, address: Pubkey) -> Option<u64> {
        let (found, lamports) = self.backend.apply_to_solana_account(
            &address,
            || (false, u64::default()),
            |info| (true, info.lamports),
        );
        if found { Some(lamports) } else { None }
    }

    #[must_use]
    pub fn query_solana_account_executable(&self, address: Pubkey) -> Option<bool> {
        let (found, executable) = self.backend.apply_to_solana_account(
            &address,
            || (false, bool::default()),
            |info| (true, info.executable),
        );
        if found { Some(executable) } else { None }
    }

    #[must_use]
    pub fn query_solana_account_rent_epoch(&self, address: Pubkey) -> Option<u64> {
        let (found, rent_epoch) = self.backend.apply_to_solana_account(
            &address,
            || (false, u64::default()),
            |info| (true, info.rent_epoch),
        );
        if found { Some(rent_epoch) } else { None }
    }

    #[must_use]
    pub fn query_solana_account_data(&self, address: Pubkey, offset: usize, length: usize) -> Option<Vec<u8>> {
        fn clone_chunk(data: &[u8], offset: usize, length: usize) -> Option<Vec<u8>> {
            if offset >= data.len() || offset + length > data.len() {
                None
            } else {
                Some(data[offset..offset + length].to_owned())
            }
        }
        self.backend.apply_to_solana_account(
            &address,
            || None,
            |info| clone_chunk(&info.data.borrow(), offset, length)
        )
    }

    #[must_use]
    pub fn gasometer_mut(&mut self) -> &mut Gasometer {
        &mut self.substate.metadata.gasometer
    }

    #[must_use]
    pub fn gasometer(&self) -> &Gasometer {
        self.substate.metadata().gasometer()
    }

    pub fn new(substate: Box<ExecutorSubstate>, backend: &'a B) -> Self {
        Self { backend, substate }
    }

    #[must_use]
    pub fn substate(&self) -> &ExecutorSubstate {
        &self.substate
    }

    #[must_use]
    pub fn backend(&self) -> &'a B {
        self.backend
    }

    #[must_use]
    pub fn deconstruct(
        self,
    ) -> ApplyState {
        self.substate.deconstruct(self.backend)
    }
}
