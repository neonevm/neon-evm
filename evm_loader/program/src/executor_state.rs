use std::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec::Vec
};
use core::mem;
use evm::backend::{Apply, Backend, Basic, Log};
use evm::{ExitError, Transfer};
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExecutorAccount {
    pub basic: Basic,
    pub code: Option<Vec<u8>>,
    pub reset: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutorMetadata {
    // gasometer: Gasometer<'config>,
    is_static: bool,
    depth: Option<usize>
}

impl ExecutorMetadata {
    pub fn new() -> Self {
        Self {
            // gasometer: Gasometer::new(gas_limit, config),
            is_static: false,
            depth: None
        }
    }

    pub fn swallow_commit(&mut self, other: Self) -> Result<(), ExitError> {
        // self.gasometer.record_stipend(other.gasometer.gas())?;
        // self.gasometer
        //     .record_refund(other.gasometer.refunded_gas())?;

        // if let Some(runtime) = self.runtime.borrow_mut().as_ref() {
        //     let return_value = other.borrow().runtime().unwrap().machine().return_value();
        //     runtime.set_return_data(return_value);
        // }

        Ok(())
    }

    pub fn swallow_revert(&mut self, other: Self) -> Result<(), ExitError> {
        // self.gasometer.record_stipend(other.gasometer.gas())?;

        Ok(())
    }

    pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
        Ok(())
    }

    pub fn spit_child(&self, gas_limit: u64, is_static: bool) -> Self {
        Self {
            // gasometer: Gasometer::new(gas_limit, self.gasometer.config()),
            is_static: is_static || self.is_static,
            depth: match self.depth {
                None => Some(0),
                Some(n) => Some(n + 1),
            }
        }
    }

    // pub fn gasometer(&self) -> &Gasometer {
    //     &self.gasometer
    // }

    // pub fn gasometer_mut(&mut self) -> &mut Gasometer {
    //     &mut self.gasometer
    // }

    pub fn is_static(&self) -> bool {
        self.is_static
    }

    pub fn depth(&self) -> Option<usize> {
        self.depth
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutorSubstate {
    metadata: ExecutorMetadata,
    parent: Option<Box<ExecutorSubstate>>,
    logs: Vec<Log>,
    accounts: BTreeMap<H160, ExecutorAccount>,
    storages: BTreeMap<(H160, U256), U256>,
    deletes: BTreeSet<H160>,
}

impl ExecutorSubstate {
    pub fn new() -> Self {
        Self {
            metadata: ExecutorMetadata::new(),
            parent: None,
            logs: Vec::new(),
            accounts: BTreeMap::new(),
            storages: BTreeMap::new(),
            deletes: BTreeSet::new(),
        }
    }

    pub fn metadata(&self) -> &ExecutorMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut ExecutorMetadata {
        &mut self.metadata
    }

    /// Deconstruct the executor, return state to be applied. Panic if the
    /// executor is not in the top-level substate.
    #[must_use]
    pub fn deconstruct<B: Backend>(
        mut self,
        backend: &B,
    ) -> (Vec::<Apply<BTreeMap<U256, U256>>>, Vec<Log>) {
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
                let account = self.account_mut(address, backend);

                Apply::Modify {
                    address,
                    basic: account.basic.clone(),
                    code: account.code.clone(),
                    storage,
                    reset_storage: account.reset,
                }
            };

            applies.push(apply);
        }

        for address in self.deletes {
            applies.push(Apply::Delete { address });
        }

        (applies, self.logs)
    }

    pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
        let mut entering = Self {
            metadata: self.metadata.spit_child(gas_limit, is_static),
            parent: None,
            logs: Vec::new(),
            accounts: BTreeMap::new(),
            storages: BTreeMap::new(),
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

        let mut resets = BTreeSet::new();
        for (address, account) in &exited.accounts {
            if account.reset {
                resets.insert(*address);
            }
        }
        let mut reset_keys = BTreeSet::new();
        for (address, key) in self.storages.keys() {
            if resets.contains(&address) {
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

        for (address) in &resets {
            if self.accounts.contains_key(address){
                self.accounts.get_mut(&address).unwrap().reset = true;
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
        if let Some(account) = self.accounts.get(&address) {
            Some(account)
        } else if let Some(parent) = self.parent.as_ref() {
            parent.known_account(address)
        } else {
            None
        }
    }

    pub fn known_basic(&self, address: H160) -> Option<Basic> {
        self.known_account(address).map(|acc| acc.basic.clone())
    }

    pub fn known_code(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.code.clone())
    }

    pub fn known_empty(&self, address: H160) -> Option<bool> {
        if let Some(account) = self.known_account(address) {
            if account.basic.balance != U256::zero() {
                return Some(false);
            }

            if account.basic.nonce != U256::zero() {
                return Some(false);
            }

            if let Some(code) = &account.code {
                return Some(
                    account.basic.balance == U256::zero()
                        && account.basic.nonce == U256::zero()
                        && code.len() == 0,
                );
            }
        }

        None
    }

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

    pub fn deleted(&self, address: H160) -> bool {
        if self.deletes.contains(&address) {
            return true;
        }

        if let Some(parent) = self.parent.as_ref() {
            return parent.deleted(address);
        }

        false
    }

    fn account_mut<B: Backend>(&mut self, address: H160, backend: &B) -> &mut ExecutorAccount {
        if !self.accounts.contains_key(&address) {
            let account = self
                .known_account(address)
                .cloned()
                .map(|mut v| {
                    v.reset = false;
                    v
                })
                .unwrap_or_else(|| ExecutorAccount {
                    basic: backend.basic(address),
                    code: None,
                    reset: false,
                });
            self.accounts.insert(address, account);
        }

        self.accounts
            .get_mut(&address)
            .expect("New account was just inserted")
    }

    pub fn inc_nonce<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend).basic.nonce += U256::one();
    }

    pub fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        self.storages.insert((address, key), value);
    }

    pub fn reset_storage<B: Backend>(&mut self, address: H160, backend: &B) {
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

    pub fn set_code<B: Backend>(&mut self, address: H160, code: Vec<u8>, backend: &B) {
        self.account_mut(address, backend).code = Some(code);
    }

    pub fn transfer<B: Backend>(
        &mut self,
        transfer: Transfer,
        backend: &B,
    ) -> Result<(), ExitError> {
        {
            let source = self.account_mut(transfer.source, backend);
            if source.basic.balance < transfer.value {
                return Err(ExitError::OutOfFund);
            }
            source.basic.balance -= transfer.value;
        }

        {
            let target = self.account_mut(transfer.target, backend);
            target.basic.balance = target.basic.balance.saturating_add(transfer.value);
        }

        Ok(())
    }

    // Only needed for jsontests.
    pub fn withdraw<B: Backend>(
        &mut self,
        address: H160,
        value: U256,
        backend: &B,
    ) -> Result<(), ExitError> {
        let source = self.account_mut(address, backend);
        if source.basic.balance < value {
            return Err(ExitError::OutOfFund);
        }
        source.basic.balance -= value;

        Ok(())
    }

    // Only needed for jsontests.
    pub fn deposit<B: Backend>(&mut self, address: H160, value: U256, backend: &B) {
        let target = self.account_mut(address, backend);
        target.basic.balance = target.basic.balance.saturating_add(value);
    }

    pub fn reset_balance<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend).basic.balance = U256::zero();
    }

    pub fn touch<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend);
    }
}

pub trait StackState : Backend {
    fn metadata(&self) -> &ExecutorMetadata;
    fn metadata_mut(&mut self) -> &mut ExecutorMetadata;

    fn enter(&mut self, gas_limit: u64, is_static: bool);
    fn exit_commit(&mut self) -> Result<(), ExitError>;
    fn exit_revert(&mut self) -> Result<(), ExitError>;
    fn exit_discard(&mut self) -> Result<(), ExitError>;

    fn is_empty(&self, address: H160) -> bool;
    fn deleted(&self, address: H160) -> bool;

    fn inc_nonce(&mut self, address: H160);
    fn set_storage(&mut self, address: H160, key: U256, value: U256);
    fn reset_storage(&mut self, address: H160);
    fn original_storage(&self, address: H160, key: U256) -> Option<U256>;
    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>);
    fn set_deleted(&mut self, address: H160);
    fn set_code(&mut self, address: H160, code: Vec<u8>);
    fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError>;
    fn reset_balance(&mut self, address: H160);
    fn touch(&mut self, address: H160);
}

pub struct ExecutorState<B: Backend> {
    backend: B,
    substate: ExecutorSubstate,
}

impl<B: Backend> Backend for ExecutorState<B> {
    fn gas_price(&self) -> U256 {
        self.backend.gas_price()
    }
    fn origin(&self) -> H160 {
        self.backend.origin()
    }
    fn block_hash(&self, number: U256) -> H256 {
        self.backend.block_hash(number)
    }
    fn block_number(&self) -> U256 {
        self.backend.block_number()
    }
    fn block_coinbase(&self) -> H160 {
        self.backend.block_coinbase()
    }
    fn block_timestamp(&self) -> U256 {
        self.backend.block_timestamp()
    }
    fn block_difficulty(&self) -> U256 {
        self.backend.block_difficulty()
    }
    fn block_gas_limit(&self) -> U256 {
        self.backend.block_gas_limit()
    }
    fn chain_id(&self) -> U256 {
        self.backend.chain_id()
    }

    fn exists(&self, address: H160) -> bool {
        self.substate.known_account(address).is_some() || self.backend.exists(address)
    }

    fn basic(&self, address: H160) -> Basic {
        self.substate
            .known_basic(address)
            .unwrap_or_else(|| self.backend.basic(address))
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.substate
            .known_code(address)
            .unwrap_or_else(|| self.backend.code(address))
    }

    fn code_hash(&self, address: H160) -> H256 {
        self.substate.known_code(address)
            .map(|code| Keccak256::digest(&code))
            .map(|digest| H256::from_slice(digest.as_slice()))
            .unwrap_or(self.backend.code_hash(address))
    }

    fn code_size(&self, address: H160) -> usize {
         self.substate.known_code(address)
            .map(|code| code.len())
            .unwrap_or(self.backend.code_size(address))
    }

    fn storage(&self, address: H160, key: U256) -> U256 {
        self.substate
            .known_storage(address, key)
            .unwrap_or_else(|| self.backend.storage(address, key))
    }

    fn create(&self, scheme: &evm::CreateScheme, address: &H160) {
        self.backend.create(scheme, address)
    }

    fn call_inner(&self, // todo remove
        code_address: H160,
        transfer: Option<evm::Transfer>,
        input: Vec<u8>,
        target_gas: Option<usize>,
        is_static: bool,
        take_l64: bool,
        take_stipend: bool,
    ) -> Option<evm::Capture<(evm::ExitReason, Vec<u8>), std::convert::Infallible>> {
        self.backend.call_inner(code_address, transfer, input, target_gas, is_static, take_l64, take_stipend)
    }
}

impl<B: Backend> StackState for ExecutorState<B> {
    fn metadata(&self) -> &ExecutorMetadata {
        self.substate.metadata()
    }

    fn metadata_mut(&mut self) -> &mut ExecutorMetadata {
        self.substate.metadata_mut()
    }

    fn enter(&mut self, gas_limit: u64, is_static: bool) {
        self.substate.enter(gas_limit, is_static)
    }

    fn exit_commit(&mut self) -> Result<(), ExitError> {
        self.substate.exit_commit()
    }

    fn exit_revert(&mut self) -> Result<(), ExitError> {
        self.substate.exit_revert()
    }

    fn exit_discard(&mut self) -> Result<(), ExitError> {
        self.substate.exit_discard()
    }

    fn is_empty(&self, address: H160) -> bool {
        if let Some(known_empty) = self.substate.known_empty(address) {
            return known_empty;
        }

        self.backend.basic(address).balance == U256::zero()
            && self.backend.basic(address).nonce == U256::zero()
            && self.backend.code(address).len() == 0
    }

    fn deleted(&self, address: H160) -> bool {
        self.substate.deleted(address)
    }

    fn inc_nonce(&mut self, address: H160) {
        self.substate.inc_nonce(address, &self.backend);
    }

    fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        self.substate.set_storage(address, key, value)
    }

    fn reset_storage(&mut self, address: H160) {
        self.substate.reset_storage(address, &self.backend);
    }

    fn original_storage(&self, address: H160, key: U256) -> Option<U256> {
        if let Some(value) = self.substate.known_original_storage(address, key) {
            return Some(value);
        }

        Some(self.backend.storage(address, key)) // todo backend.original_storage
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.substate.log(address, topics, data);
    }

    fn set_deleted(&mut self, address: H160) {
        self.substate.set_deleted(address)
    }

    fn set_code(&mut self, address: H160, code: Vec<u8>) {
        self.substate.set_code(address, code, &self.backend)
    }

    fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
        self.substate.transfer(transfer, &self.backend)
    }

    fn reset_balance(&mut self, address: H160) {
        self.substate.reset_balance(address, &self.backend)
    }

    fn touch(&mut self, address: H160) {
        self.substate.touch(address, &self.backend)
    }
}

impl<B: Backend> ExecutorState<B> {
    pub fn new(substate: ExecutorSubstate, backend: B) -> Self {
        Self {
            backend,
            substate,
        }
    }

    pub fn substate(&self) -> &ExecutorSubstate {
        &self.substate
    }

    #[must_use]
    pub fn deconstruct(
        self,
    ) -> (B, (Vec::<Apply<BTreeMap<U256, U256>>>, Vec<Log>)) {
        let (applies, logs) = self.substate.deconstruct(&self.backend);
        (self.backend, (applies, logs))
    }

    pub fn withdraw(&mut self, address: H160, value: U256) -> Result<(), ExitError> {
        self.substate.withdraw(address, value, &self.backend)
    }

    pub fn deposit(&mut self, address: H160, value: U256) {
        self.substate.deposit(address, value, &self.backend)
    }
}