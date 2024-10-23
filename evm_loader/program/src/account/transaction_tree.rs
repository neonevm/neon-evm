use std::cell::{Ref, RefMut};
use std::mem::size_of;

use super::program::System;
use super::treasury::Treasury;
use super::{
    AccountHeader, AccountsDB, BalanceAccount, ACCOUNT_PREFIX_LEN, ACCOUNT_SEED_VERSION,
    TAG_TRANSACTION_TREE,
};
use crate::config::TREE_ACCOUNT_TIMEOUT;
use crate::error::{Error, Result};
use crate::evm::ExitStatus;
use crate::types::{Address, Transaction, TransactionPayload};
use ethnum::U256;
use solana_program::{
    account_info::AccountInfo, clock::Clock, pubkey::Pubkey, rent::Rent, system_program,
    sysvar::Sysvar,
};

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Status {
    Failed = 0x00,
    Success = 0x01,
    Skipped = 0x02,
    InProgress = 0x03,
    #[default]
    NotStarted = 0xFF,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Node {
    pub status: Status,

    pub result_hash: [u8; 32],
    pub transaction_hash: [u8; 32],

    pub gas_limit: U256,
    pub value: U256,

    pub child_transaction: u16,
    pub success_execute_limit: u16,
    pub parent_count: u16,
}
static_assertions::assert_eq_size!(Node, [u8; 135]);

pub const NO_CHILD_TRANSACTION: u16 = u16::MAX;

#[repr(C, packed)]
pub struct HeaderV0 {
    payer: Address,
    last_slot: u64,
    chain_id: u64,
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    balance: U256,
    last_index: u16,
}
static_assertions::assert_eq_size!(HeaderV0, [u8; 134]);

impl AccountHeader for HeaderV0 {
    const VERSION: u8 = 0;
}

// Set the last version of the Header struct here
// and change the `header_size` and `header_upgrade` functions
pub type Header = HeaderV0;

pub struct NodeInitializer {
    pub transaction_hash: [u8; 32],
    pub child: u16,
    pub success_execute_limit: u16,
    pub gas_limit: U256,
    pub value: U256,
}

pub struct TreeInitializer {
    pub payer: Address,
    pub nonce: u64,
    pub chain_id: u64,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub nodes: Vec<NodeInitializer>,
}

pub struct TransactionTree<'a> {
    account: AccountInfo<'a>,
}

impl<'a> TransactionTree<'a> {
    #[must_use]
    pub fn required_account_size(transactions: usize) -> usize {
        ACCOUNT_PREFIX_LEN + size_of::<Header>() + transactions * size_of::<Node>()
    }

    #[must_use]
    pub fn required_header_realloc(&self) -> usize {
        let allocated_header_size = self.header_size();
        size_of::<Header>().saturating_sub(allocated_header_size)
    }

    pub fn from_account(program_id: &Pubkey, account: AccountInfo<'a>) -> Result<Self> {
        super::validate_tag(program_id, &account, TAG_TRANSACTION_TREE)?;

        Ok(Self { account })
    }

    #[must_use]
    pub fn info(&self) -> &AccountInfo<'a> {
        &self.account
    }

    #[must_use]
    pub fn find_address(program_id: &Pubkey, payer: Address, nonce: u64) -> (Pubkey, u8) {
        let seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            b"TREE",
            payer.as_bytes(),
            &nonce.to_le_bytes(),
        ];

        Pubkey::find_program_address(seeds, program_id)
    }

    pub fn create(
        init: TreeInitializer,
        account: AccountInfo<'a>,
        treasury: &Treasury<'a>,
        system: &System<'a>,
        rent: &Rent,
        clock: &Clock,
    ) -> Result<Self> {
        // Validate account
        let (pubkey, bump_seed) = Self::find_address(&crate::ID, init.payer, init.nonce);
        if account.key != &pubkey {
            return Err(Error::AccountInvalidKey(*account.key, pubkey));
        }

        if account.owner != &system_program::ID {
            return Err(Error::AccountInvalidOwner(*account.key, system_program::ID));
        }

        let seeds: &[&[u8]] = &[
            &[ACCOUNT_SEED_VERSION],
            b"TREE",
            init.payer.as_bytes(),
            &init.nonce.to_le_bytes(),
            &[bump_seed],
        ];

        // Validate init data
        let nodes = init.nodes;
        let mut parent_counts = vec![0_u16; nodes.len()];

        for node in &nodes {
            if node.child == NO_CHILD_TRANSACTION {
                continue;
            }

            if node.child as usize >= nodes.len() {
                return Err(Error::TreeAccountTxInvalidChildIndex);
            }

            parent_counts[node.child as usize] += 1;
        }

        for (node, parent_count) in nodes.iter().zip(&parent_counts) {
            if node.success_execute_limit > *parent_count {
                return Err(Error::TreeAccountTxInvalidSuccessLimit);
            }
        }

        // Create account
        let space = Self::required_account_size(nodes.len());
        system.create_pda_account_with_treasury_payer(
            &crate::ID,
            treasury,
            &account,
            seeds,
            space,
            rent,
        )?;

        // Init data
        super::set_tag(&crate::ID, &account, TAG_TRANSACTION_TREE, Header::VERSION)?;
        let mut tree = Self::from_account(&crate::ID, account)?;

        {
            let mut header = super::header_mut::<HeaderV0>(&tree.account);
            header.payer = init.payer;
            header.last_slot = clock.slot;
            header.chain_id = init.chain_id;
            header.max_fee_per_gas = init.max_fee_per_gas;
            header.max_priority_fee_per_gas = init.max_priority_fee_per_gas;
            header.balance = U256::ZERO;
            header.last_index = nodes.len().try_into()?;
        }

        let init_nodes = nodes.into_iter().zip(parent_counts);
        for (node, (init, parent_count)) in tree.nodes_mut().iter_mut().zip(init_nodes) {
            node.status = Status::NotStarted;
            node.result_hash = [0; 32];
            node.transaction_hash = init.transaction_hash;
            node.gas_limit = init.gas_limit;
            node.value = init.value;
            node.child_transaction = init.child;
            node.success_execute_limit = init.success_execute_limit;
            node.parent_count = parent_count;
        }

        Ok(tree)
    }

    #[must_use]
    pub fn is_in_progress(&self) -> bool {
        self.nodes()
            .iter()
            .any(|n| matches!(n.status, Status::InProgress))
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.nodes()
            .iter()
            .all(|n| !matches!(n.status, Status::InProgress | Status::NotStarted))
    }

    #[must_use]
    pub fn is_not_started(&self) -> bool {
        self.nodes()
            .iter()
            .all(|n| matches!(n.status, Status::NotStarted))
    }

    #[must_use]
    pub fn can_be_destroyed(&self, clock: &Clock) -> bool {
        if self.balance() != U256::ZERO {
            return false;
        }

        if self.is_in_progress() {
            return false;
        }

        if self.last_slot() < clock.slot.saturating_sub(TREE_ACCOUNT_TIMEOUT) {
            return true;
        }

        self.is_complete()
    }

    pub fn destroy(self, treasury: &Treasury<'a>) -> Result<()> {
        let clock = Clock::get()?;

        if !self.can_be_destroyed(&clock) {
            return Err(Error::TreeAccountNotReadyForDestruction);
        }

        unsafe { super::delete_with_treasury(&self.account, treasury) }
    }

    fn validate_transaction(&self, tx: &Transaction) -> Result<u16> {
        let hash = tx.hash;

        let TransactionPayload::Scheduled(tx) = &tx.transaction else {
            return Err(Error::TreeAccountTxInvalidType);
        };

        let (pubkey, _) = Self::find_address(&crate::ID, tx.payer, tx.nonce);
        if &pubkey != self.account.key {
            return Err(Error::TreeAccountTxInvalidDataPubkey);
        }

        if tx.index as usize >= self.nodes().len() {
            return Err(Error::TreeAccountTxInvalidDataIndex);
        }

        let node = self.node(tx.index);
        if node.transaction_hash != hash {
            return Err(Error::TreeAccountTxInvalidDataHash);
        }

        let gas_limit = node.gas_limit; // Copy from unaligned
        if gas_limit != tx.gas_limit {
            return Err(Error::TreeAccountTxInvalidDataGas);
        }
        let value = node.value;
        if value != tx.value {
            return Err(Error::TreeAccountTxInvalidDataValue);
        }

        if tx.payer != self.payer() {
            return Err(Error::TreeAccountTxInvalidDataPayer);
        }

        if tx.chain_id != U256::from(self.chain_id()) {
            return Err(Error::TreeAccountTxInvalidDataChainId);
        }

        if tx.max_fee_per_gas != self.max_fee_per_gas() {
            return Err(Error::TreeAccountTxInvalidDataFee);
        }

        if tx.max_priority_fee_per_gas != self.max_priority_fee_per_gas() {
            return Err(Error::TreeAccountTxInvalidDataPriorityFee);
        }

        // We don't support intents at the moment
        if tx.intent.is_some() {
            return Err(Error::TreeAccountTxInvalidDataIntent);
        }

        if !tx.intent_call_data.is_empty() {
            return Err(Error::TreeAccountTxInvalidDataCallData);
        }

        Ok(tx.index)
    }

    pub fn start_transaction(&mut self, tx: &Transaction) -> Result<()> {
        let index = self.validate_transaction(tx)?;
        let mut node = self.node_mut(index);

        if node.status != Status::NotStarted {
            return Err(Error::TreeAccountTxInvalidStatus);
        }
        if node.parent_count != 0 {
            return Err(Error::TreeAccountTxInvalidParentCount);
        }
        if node.success_execute_limit != 0 {
            return Err(Error::TreeAccountTxInvalidSuccessLimit);
        }

        node.status = Status::InProgress;

        Ok(())
    }

    pub fn skip_transaction(&mut self, tx: &Transaction) -> Result<()> {
        let index = self.validate_transaction(tx)?;
        let mut node = self.node_mut(index);

        if node.status != Status::NotStarted {
            return Err(Error::TreeAccountTxInvalidStatus);
        }
        if node.parent_count != 0 {
            return Err(Error::TreeAccountTxInvalidParentCount);
        }
        if node.success_execute_limit == 0 {
            // Transaction need to be started
            return Err(Error::TreeAccountTxInvalidSuccessLimit);
        }

        node.status = Status::Skipped;

        let child_index = node.child_transaction;
        std::mem::drop(node);

        self.decrease_parent_count(child_index, Status::Skipped);

        Ok(())
    }

    pub fn end_transaction(&mut self, index: u16, result: &ExitStatus) -> Result<()> {
        use solana_program::keccak::{hash as keccak256, Hash};

        let mut node = self.node_mut(index);

        if node.status != Status::InProgress {
            return Err(Error::TreeAccountTxInvalidStatus);
        }

        let (status, Hash(result_hash)) = match result {
            ExitStatus::Stop | ExitStatus::Suicide => (Status::Success, keccak256(&[])),
            ExitStatus::Return(result) => (Status::Success, keccak256(result)),
            ExitStatus::Revert(result) => (Status::Failed, keccak256(result)),
            ExitStatus::StepLimit => {
                panic!("Tree Account transaction can't be ended with StepLimit")
            }
        };

        node.status = status;
        node.result_hash = result_hash;

        let child_index = node.child_transaction;
        std::mem::drop(node);

        self.decrease_parent_count(child_index, status);

        Ok(())
    }

    #[must_use]
    pub fn payer(&self) -> Address {
        let header = super::header::<HeaderV0>(&self.account);
        header.payer
    }

    #[must_use]
    pub fn last_slot(&self) -> u64 {
        let header = super::header::<HeaderV0>(&self.account);
        header.last_slot
    }

    pub fn update_last_slot(&mut self, clock: &Clock) {
        let mut header = super::header_mut::<HeaderV0>(&self.account);
        header.last_slot = clock.slot;
    }

    #[must_use]
    pub fn chain_id(&self) -> u64 {
        let header = super::header::<HeaderV0>(&self.account);
        header.chain_id
    }

    #[must_use]
    pub fn max_fee_per_gas(&self) -> U256 {
        let header = super::header::<HeaderV0>(&self.account);
        header.max_fee_per_gas
    }

    #[must_use]
    pub fn max_priority_fee_per_gas(&self) -> U256 {
        let header = super::header::<HeaderV0>(&self.account);
        header.max_priority_fee_per_gas
    }

    #[must_use]
    pub fn total_gas_limit(&self) -> U256 {
        self.nodes()
            .iter()
            .fold(U256::ZERO, |v, node| v.saturating_add(node.gas_limit))
    }

    #[must_use]
    pub fn total_value(&self) -> U256 {
        self.nodes()
            .iter()
            .fold(U256::ZERO, |v, node| v.saturating_add(node.value))
    }

    #[must_use]
    pub fn balance(&self) -> U256 {
        let header = super::header::<HeaderV0>(&self.account);
        header.balance
    }

    pub fn withdraw(&mut self, target: &mut BalanceAccount) -> Result<()> {
        assert_eq!(self.chain_id(), target.chain_id());
        assert_eq!(self.payer(), target.address());

        let value = self.balance();

        self.burn(value)?;
        target.mint(value)
    }

    pub fn burn(&mut self, value: U256) -> Result<()> {
        let mut header = super::header_mut::<HeaderV0>(&self.account);

        header.balance = header
            .balance
            .checked_sub(value)
            .ok_or(Error::InsufficientBalance(
                header.payer,
                header.chain_id,
                value,
            ))?;

        Ok(())
    }

    pub fn mint(&mut self, value: U256) -> Result<()> {
        let mut header = super::header_mut::<HeaderV0>(&self.account);

        header.balance = header
            .balance
            .checked_add(value)
            .ok_or(Error::IntegerOverflow)?;

        Ok(())
    }

    #[must_use]
    pub fn last_index(&self) -> u16 {
        let header = super::header::<HeaderV0>(&self.account);
        header.last_index
    }

    pub fn increment_last_index(&mut self) -> Result<()> {
        let mut header = super::header_mut::<HeaderV0>(&self.account);
        header.last_index = header
            .last_index
            .checked_add(1)
            .ok_or(Error::TreeAccountLastIndexOverflow)?;

        Ok(())
    }

    fn header_size(&self) -> usize {
        match super::header_version(&self.account) {
            0 | 1 => size_of::<HeaderV0>(),
            _ => panic!("Unknown header version"),
        }
    }

    #[allow(unused)]
    fn header_upgrade(&mut self, rent: &Rent, db: &AccountsDB<'a>) -> Result<()> {
        match super::header_version(&self.account) {
            0 | 1 => {
                super::expand_header::<HeaderV0, Header>(&self.account, rent, db)?;
            }
            _ => panic!("Unknown header version"),
        }

        Ok(())
    }

    fn nodes_offset(&self) -> usize {
        ACCOUNT_PREFIX_LEN + self.header_size()
    }

    #[must_use]
    pub fn nodes(&self) -> Ref<[Node]> {
        let nodes_offset = self.nodes_offset();

        let data = self.account.data.borrow();
        let data = Ref::map(data, |d| &d[nodes_offset..]);

        Ref::map(data, |bytes| {
            static_assertions::assert_eq_align!(Node, u8);
            assert_eq!(bytes.len() % size_of::<Node>(), 0);

            // SAFETY: Node has the same alignment as bytes
            unsafe {
                let ptr = bytes.as_ptr().cast::<Node>();
                let len = bytes.len() / size_of::<Node>();
                std::slice::from_raw_parts(ptr, len)
            }
        })
    }

    #[must_use]
    pub fn nodes_mut(&mut self) -> RefMut<[Node]> {
        let nodes_offset = self.nodes_offset();

        let data = self.account.data.borrow_mut();
        let data = RefMut::map(data, |d| &mut d[nodes_offset..]);

        RefMut::map(data, |bytes| {
            static_assertions::assert_eq_align!(Node, u8);
            assert_eq!(bytes.len() % size_of::<Node>(), 0);

            // SAFETY: Node has the same alignment as bytes
            unsafe {
                let ptr = bytes.as_mut_ptr().cast::<Node>();
                let len = bytes.len() / size_of::<Node>();
                std::slice::from_raw_parts_mut(ptr, len)
            }
        })
    }

    #[must_use]
    pub fn node(&self, index: u16) -> Ref<Node> {
        let nodes = self.nodes();
        Ref::map(nodes, |nodes| &nodes[index as usize])
    }

    #[must_use]
    pub fn node_mut(&mut self, index: u16) -> RefMut<Node> {
        let nodes = self.nodes_mut();
        RefMut::map(nodes, |nodes| &mut nodes[index as usize])
    }

    pub fn find_node(&self, hash: [u8; 32]) -> Result<u16> {
        let nodes = self.nodes();
        let index = nodes
            .iter()
            .position(|node| node.transaction_hash == hash)
            .ok_or(Error::TreeAccountTxNotFound)?;

        let index: u16 = index.try_into()?;
        Ok(index)
    }

    fn decrease_parent_count(&mut self, index: u16, parent_status: Status) {
        if index == NO_CHILD_TRANSACTION {
            return;
        }

        let mut child = self.nodes_mut()[index as usize];
        let new_parent_count = child.parent_count.checked_sub(1);
        child.parent_count = new_parent_count.unwrap(); // Parent count is calculated by us when tree is created. If code is correct, this should never panic

        if parent_status == Status::Success {
            child.success_execute_limit = child.success_execute_limit.saturating_sub(1);
        }
    }
}
