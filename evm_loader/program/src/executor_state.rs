//! # Neon EVM Executor State
//!
//! Executor State is a struct that stores the state during execution.

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
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

use crate::{
    query,
    account_storage::AccountStorage,
    utils::keccak256_h256
};

use spl_associated_token_account::get_associated_token_address;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExecutorAccount {
    pub nonce: U256,
    #[serde(with = "serde_bytes")]
    pub code: Option<Vec<u8>>,
    #[serde(with = "serde_bytes")]
    pub valids: Option<Vec<u8>>,
    pub reset: bool,
}

/// Represents additional data attached to an executor.
#[derive(Serialize, Deserialize)]
pub struct ExecutorMetadata {
    is_static: bool,
    depth: Option<usize>,
    block_number: U256,
    block_timestamp: U256,
}

impl ExecutorMetadata {
    /// Creates new empty metadata with specified gas limit.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new<B: AccountStorage>(backend: &B) -> Self {
        Self {
            is_static: false,
            depth: None,
            block_number: backend.block_number(),
            block_timestamp: backend.block_timestamp()
        }
    }

    #[allow(clippy::needless_pass_by_value, clippy::unused_self)]
    pub fn swallow_commit(&mut self, _other: Self) -> Result<(), ExitError> {
    	// The following fragment deleted in the mainstream code:
        // if let Some(runtime) = self.runtime.borrow_mut().as_ref() {
        //     let return_value = other.borrow().runtime().unwrap().machine().return_value();
        //     runtime.set_return_data(return_value);
        // }

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value, clippy::unused_self)]
    pub fn swallow_revert(&mut self, _other: Self) -> Result<(), ExitError> {
        Ok(())
    }

    /// Records gas usage on discard (actually does nothing).
    /// # Errors
    /// Cannot return an error.
    #[allow(clippy::needless_pass_by_value, clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
        Ok(())
    }

    /// Creates new instance of metadata when entering next frame of execution.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn spit_child(&self, is_static: bool) -> Self {
        Self {
            is_static: is_static || self.is_static,
            depth: match self.depth {
                None => Some(0),
                Some(n) => Some(n + 1),
            },
            block_number: self.block_number,
            block_timestamp: self.block_timestamp,
        }
    }

    /// Returns property `is_static`.
    #[must_use]
    pub const fn is_static(&self) -> bool {
        self.is_static
    }

    /// Returns current depth of frame of execution.
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Withdraw {
    pub source: H160,
    pub dest: Pubkey,
    pub dest_neon: Pubkey,
    pub neon_amount: U256,
    pub spl_amount: u64
}

/// Represents the state of executor abstracted away from a backend.
#[derive(Serialize, Deserialize)]
pub struct ExecutorSubstate {
    metadata: ExecutorMetadata,
    parent: Option<Box<ExecutorSubstate>>,
    logs: Vec<Log>,
    transfers: Vec<Transfer>,
    block_hashes: RefCell<BTreeMap<U256, H256>>,
    accounts: BTreeMap<H160, ExecutorAccount>,
    balances: RefCell<BTreeMap<H160, U256>>,
    storages: BTreeMap<(H160, U256), U256>,
    spl_balances: RefCell<BTreeMap<Pubkey, u64>>,
    spl_decimals: RefCell<BTreeMap<Pubkey, u8>>,
    spl_supply: RefCell<BTreeMap<Pubkey, u64>>,
    spl_transfers: Vec<SplTransfer>,
    spl_approves: Vec<SplApprove>,
    withdrawals: Vec<Withdraw>,
    erc20_allowances: RefCell<BTreeMap<(H160, H160, H160, Pubkey), U256>>,
    deletes: BTreeSet<H160>,
    query_account_cache: query::AccountCache,
}

pub type ApplyState = (Vec::<Apply<BTreeMap<U256, U256>>>, Vec<Log>, Vec<Transfer>, Vec<SplTransfer>, Vec<SplApprove>, Vec<Withdraw>, Vec<ERC20Approve>);

impl ExecutorSubstate {
    /// Creates new empty instance of `ExecutorSubstate`.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new<B: AccountStorage>(backend: &B) -> Self {
        Self {
            metadata: ExecutorMetadata::new(backend),
            parent: None,
            logs: Vec::new(),
            transfers: Vec::new(),
            block_hashes: RefCell::new(BTreeMap::new()),
            accounts: BTreeMap::new(),
            balances: RefCell::new(BTreeMap::new()),
            storages: BTreeMap::new(),
            spl_balances: RefCell::new(BTreeMap::new()),
            spl_decimals: RefCell::new(BTreeMap::new()),
            spl_supply: RefCell::new(BTreeMap::new()),
            spl_transfers: Vec::new(),
            spl_approves: Vec::new(),
            withdrawals: Vec::new(),
            erc20_allowances: RefCell::new(BTreeMap::new()),
            deletes: BTreeSet::new(),
            query_account_cache: query::AccountCache::new(),
        }
    }

    /// Returns an immutable reference on executor metadata.
    #[must_use]
    pub const fn metadata(&self) -> &ExecutorMetadata {
        &self.metadata
    }

    /// Returns the mutable reference on executor metadata.
    pub fn metadata_mut(&mut self) -> &mut ExecutorMetadata {
        &mut self.metadata
    }

    /// Deconstructs the executor, returns state to be applied.
    /// # Panics
    /// Panics if the executor is not in the top-level substate.
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

        let erc20_allowances = self.erc20_allowances.take();
        let mut erc20_approves = Vec::with_capacity(erc20_allowances.len());
        for ((owner, spender, contract, mint), value) in erc20_allowances {
            let approve = ERC20Approve { owner, spender, contract, mint, value };
            erc20_approves.push(approve);
        }

        (applies, self.logs, self.transfers, self.spl_transfers, self.spl_approves, self.withdrawals, erc20_approves)
    }

    /// Creates new instance of `ExecutorSubstate` when entering next execution of a call or create.
    pub fn enter(&mut self, is_static: bool) {
        let mut entering = Self {
            metadata: self.metadata.spit_child(is_static),
            parent: None,
            logs: Vec::new(),
            transfers: Vec::new(),
            block_hashes: RefCell::new(BTreeMap::new()),
            accounts: BTreeMap::new(),
            balances: RefCell::new(BTreeMap::new()),
            storages: BTreeMap::new(),
            spl_balances: RefCell::new(BTreeMap::new()),
            spl_decimals: RefCell::new(BTreeMap::new()),
            spl_supply: RefCell::new(BTreeMap::new()),
            spl_transfers: Vec::new(),
            spl_approves: Vec::new(),
            withdrawals: Vec::new(),
            erc20_allowances: RefCell::new(BTreeMap::new()),
            deletes: BTreeSet::new(),
            query_account_cache: query::AccountCache::new(),
        };
        mem::swap(&mut entering, self);

        self.parent = Some(Box::new(entering));
    }

    /// Commits the state on exit of call or creation.
    /// # Panics
    /// Panics on incorrect exit sequence or if an address not found in known accounts.
    /// # Errors
    /// May return one of `ExitError` variants.
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

        self.withdrawals.append(&mut exited.withdrawals);

        self.erc20_allowances.borrow_mut().append(&mut exited.erc20_allowances.borrow_mut());

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

    /// Reverts the state on exit of call or creation.
    /// # Panics
    /// Panics on incorrect exit sequence.
    /// # Errors
    /// May return one of `ExitError` variants.
    pub fn exit_revert(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_revert(exited.metadata)?;

        Ok(())
    }

    /// Discards the state on exit of call or creation.
    /// # Panics
    /// Panics on incorrect exit sequence.
    /// # Errors
    /// May return one of `ExitError` variants.
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

    /// Returns copy of basic account information if the `address` represents a known account.
    /// Returns `None` if the account is not known.
    #[must_use]
    pub fn known_nonce(&self, address: H160) -> Option<U256> {
        self.known_account(address).map(|acc| acc.nonce)
    }

    /// Returns copy of code stored in account if the `address` represents a known account.
    /// Returns `None` if the account is not known.
    #[must_use]
    pub fn known_code(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.code.clone())
    }

    /// Returns copy of `valids` bit array stored in account if the `address` represents a known account.
    /// Returns `None` if the account is not known.
    #[must_use]
    pub fn known_valids(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.valids.clone())
    }

    /// Checks if an account is empty: does not contain balance, nonce and code.
    /// Returns `None` if the account is not known.
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

    /// Returns value of record stored in a account if the `address` represents a known account.
    /// Returns zero if the account is in reset state (empty storage).
    /// Returns `None` if a record with the key does not exist or the account is not known.
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

    /// Returns zero if the account is in reset state (empty storage).
    /// Returns `None` if the account is not in reset state or is not known.
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

    /// Checks if an account has been deleted.
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

    /// Increments nonce of an account: increases it by 1.
    pub fn inc_nonce<B: AccountStorage>(&mut self, address: H160, backend: &B) {
        let account = self.account_mut(address, backend);

        let (nonce, _overflow) = account.nonce.overflowing_add(U256::one());
        account.nonce = nonce;
    }

    /// Adds or changes a record in the storage of given account.
    pub fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        self.storages.insert((address, key), value);
    }

    /// Clears the storage of an account and marks the account as reset.
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

    /// Adds an Ethereum event log record.
    pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.logs.push(Log {
            address,
            topics,
            data,
        });
    }

    /// Marks an account as deleted.
    pub fn set_deleted(&mut self, address: H160) {
        self.deletes.insert(address);
    }

    /// Initializes a contract account with it's code and corresponding bit array of valid jumps.
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

    /// Adds a transfer to execute.
    /// # Errors
    /// May return `OutOfFund` if the source has no funds.
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

    /// Resets the balance of an account: sets it to 0.
    pub fn reset_balance(&self, address: H160) {
        let mut balances = self.balances.borrow_mut();
        balances.insert(address, U256::zero());
    }

    /// Adds an account to list of known accounts if not yet added.
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

    fn withdraw<B: AccountStorage>(&mut self, withdraw: Withdraw, backend: &B) -> Result<(), ExitError> {
        debug_print!("withdraw: {:?}", withdraw);

        let new_source_balance = {
            let balance = self.balance(&withdraw.source, backend);
            balance.checked_sub(withdraw.neon_amount).ok_or(ExitError::OutOfFund)?
        };

        let new_target_balance = {
            let balance = self.spl_balance(&withdraw.dest_neon, backend);
            balance.checked_add(withdraw.spl_amount).ok_or(ExitError::InvalidRange)?
        };

        let dest_neon = withdraw.dest_neon;

        let mut balances = self.balances.borrow_mut();
        balances.insert(withdraw.source, new_source_balance);
        self.withdrawals.push(withdraw);

        let mut spl_balances = self.spl_balances.borrow_mut();
        spl_balances.insert(dest_neon, new_target_balance);

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

    fn known_erc20_allowance(&self, owner: H160, spender: H160, contract: H160, mint: Pubkey) -> Option<U256> {
        let erc20_allowances = self.erc20_allowances.borrow();
        match erc20_allowances.get(&(owner, spender, contract, mint)) {
            Some(&allowance) => Some(allowance),
            None => self.parent.as_ref().and_then(|parent| parent.known_erc20_allowance(owner, spender, contract, mint))
        }
    }

    #[must_use]
    pub fn erc20_allowance<B: AccountStorage>(&self, owner: H160, spender: H160, contract: H160, mint: Pubkey, backend: &B) -> U256 {
        let value = self.known_erc20_allowance(owner, spender, contract, mint);

        value.map_or_else(
            || {
                let allowance = backend.get_erc20_allowance(&owner, &spender, &contract, &mint);

                let key = (owner, spender, contract, mint);
                self.erc20_allowances.borrow_mut().insert(key, allowance);

                allowance
            },
            |value| value
        )
    }

    fn erc20_approve(&mut self, approve: &ERC20Approve) {
        let key = (approve.owner, approve.spender, approve.contract, approve.mint);
        self.erc20_allowances.borrow_mut().insert(key, approve.value);
    }

    fn known_block_hash(&self, number: U256) -> Option<H256> {
        let block_hashes = self.block_hashes.borrow();
        block_hashes.get(&number).copied()
    }

    #[must_use]
    pub fn block_hash<B: AccountStorage>(&self, number: U256, backend: &B) -> H256 {
        let value = self.known_block_hash(number);

        value.map_or_else(
            || {
                let block_hash = backend.block_hash(number);
                self.block_hashes.borrow_mut().insert(number, block_hash);

                block_hash
            },
            |value| value
        )
    }
}

pub struct ExecutorState<'a, B: AccountStorage> {
    backend: &'a B,
    substate: Box<ExecutorSubstate>,
}

impl<'a, B: AccountStorage> ExecutorState<'a, B> {
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn block_hash(&self, number: U256) -> H256 {
        self.substate.block_hash(number, self.backend)
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
        U256::from(self.backend.chain_id())
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

    pub fn enter(&mut self, is_static: bool) {
        self.substate.enter(is_static);
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
            let allowance = self.substate.erc20_allowance(source, context.caller, contract, mint, self.backend);
            if allowance < value {
                return false;
            }

            let approve = ERC20Approve { owner: source, spender: context.caller, contract, mint, value: allowance - value };
            self.substate.erc20_approve(&approve);
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
    pub fn erc20_allowance(&self, mint: Pubkey, context: &evm::Context, owner: H160, spender: H160) -> U256
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

    pub fn cache_solana_account(&mut self, address: Pubkey, offset: usize, length: usize) -> query::Result<()> {
        if length == 0 || length > query::MAX_CHUNK_LEN {
            return Err(query::Error::InvalidArgument);
        }
        let value = self.backend.query_account(&address, offset, length);
        match value {
            None => Err(query::Error::AccountNotFound),
            Some(value) => {
                if value.has_data() {
                    self.substate.query_account_cache.put(address, value);
                    Ok(())
                } else {
                    Err(query::Error::InvalidArgument)
                }
            }
        }
    }

    #[must_use]
    pub fn query_solana_account(&self) -> &query::AccountCache {
        &self.substate.query_account_cache
    }

    #[must_use]
    pub fn withdraw(&mut self, source: H160, destination: Pubkey, neon_amount: U256, spl_amount: u64) -> bool {
        let dest_neon_acct = get_associated_token_address(
            &destination,
            self.backend.token_mint()
        );

        let withdraw = Withdraw{
            source,
            dest: destination,
            dest_neon: dest_neon_acct,
            neon_amount,
            spl_amount
        };

        if self.substate.withdraw(withdraw, self.backend).is_err() {
            return false;
        };

        true
    }

    pub fn new(substate: Box<ExecutorSubstate>, backend: &'a B) -> Self {
        Self { backend, substate }
    }

    /// Returns an immutable reference on the executor substate.
    #[must_use]
    pub fn substate(&self) -> &ExecutorSubstate {
        &self.substate
    }

    /// Deconstructs the executor, returns state to be applied.
    /// # Panics
    /// Panics if the executor is not in the top-level substate.
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
