use std::cell::{Ref, RefCell, RefMut};
use std::mem::size_of;
use std::ops::Deref;
use std::ptr::{addr_of, slice_from_raw_parts};

use crate::account_storage::AccountStorage;
use crate::config::DEFAULT_CHAIN_ID;
use crate::debug::log_data;
use crate::error::{Error, Result};
use crate::evm::Machine;
use crate::executor::{BlockParams, ExecutorStateData};
use crate::types::boxx::{boxx, Boxx};
use crate::types::vector::VectorSliceExt;
use crate::types::{read_raw_utils::read_vec, Address, Transaction, TreeMap, TrxView, Vector};

use ethnum::U256;
use solana_program::hash::Hash;
use solana_program::system_program;
use solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey};
use static_assertions::const_assert_eq;

use super::{
    AccountHeader, AccountsDB, BalanceAccount, ContractAccount, Holder, OperatorBalanceAccount,
    StateFinalizedAccount, StorageCell, TransactionTree, TAG_ACCOUNT_BALANCE, TAG_ACCOUNT_CONTRACT,
    TAG_HOLDER, TAG_SCHEDULED_STATE_CANCELLED, TAG_SCHEDULED_STATE_FINALIZED, TAG_STATE,
    TAG_STATE_FINALIZED, TAG_STORAGE_CELL,
};

#[derive(PartialEq, Eq)]
pub enum AccountsStatus {
    Ok,
    NeedRestart,
}

#[derive(Clone, PartialEq, Eq, Copy)]
#[repr(C)]
enum AccountRevision {
    Revision(u32),
    Hash([u8; 32]),
}

impl Default for AccountRevision {
    fn default() -> Self {
        AccountRevision::Revision(0)
    }
}

impl AccountRevision {
    pub fn new(program_id: &Pubkey, info: &AccountInfo) -> Self {
        if (info.owner != program_id) && !system_program::check_id(info.owner) {
            if crate::config::NO_UPDATE_TRACKING_OWNERS
                .binary_search(info.owner)
                .is_ok()
            {
                return AccountRevision::Hash(Hash::default().to_bytes());
            }

            let hash = solana_program::hash::hashv(&[
                info.owner.as_ref(),
                &info.lamports().to_le_bytes(),
                &info.data.deref().borrow(),
            ]);

            return AccountRevision::Hash(hash.to_bytes());
        }

        match crate::account::tag(program_id, info) {
            Ok(TAG_STORAGE_CELL) => {
                let cell = StorageCell::from_account(program_id, info.clone()).unwrap();
                Self::Revision(cell.revision())
            }
            Ok(TAG_ACCOUNT_CONTRACT) => {
                let contract = ContractAccount::from_account(program_id, info.clone()).unwrap();
                Self::Revision(contract.revision())
            }
            Ok(TAG_ACCOUNT_BALANCE) => {
                let balance = BalanceAccount::from_account(program_id, info.clone()).unwrap();
                Self::Revision(balance.revision())
            }
            _ => Self::Revision(0),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct InterruptedInstruction {
    pub program_id: Pubkey,
    pub accounts: Vector<AccountMeta>,
    pub data: Vector<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct InterruptedState {
    pub instruction: InterruptedInstruction,
    pub signer_seeds: Vector<Vector<u8>>,
    pub lamports: Option<u64>,
}

type VersionSignature = [u8; 40];

#[allow(clippy::struct_field_names)]
#[repr(C, packed)]
pub struct Header {
    pub version_signature: VersionSignature,
    // Are relative offsets for the corresponding objects as allocated by the AccountAllocator.
    pub root_offset: usize,
    pub serialized_tx: std::ops::Range<usize>,
}

impl Header {
    fn valid_version_signature() -> VersionSignature {
        let mut result: VersionSignature = [0; 40];
        let state = env!("NEON_REVISION").as_bytes();
        result[..state.len()].copy_from_slice(state);
        result
    }
}

/// Storage data account to store execution metainfo between steps for iterative execution
#[repr(C)]
#[derive(Default)]
pub struct PlainData {
    // We may want to extend the PlainData, so the first field
    // indicates that some trailing fields could be filled with garbage
    // and shouldn't be reused
    pub layout_version: usize, // = 0
    pub owner: Pubkey,
    /// Ethereum transaction caller address
    pub origin: Address,
    /// Address of the tree account (present for scheduled transactions).
    pub tree_account: Option<Pubkey>,
    /// Ethereum transaction gas used and paid
    pub gas_used: U256,

    pub tx_hash: [u8; 32],
    pub chain_id: Option<u64>, // https://github.com/neonlabsorg/neon-evm/blob/develop/evm_loader/program/src/types/transaction.rs#L958

    pub tx_target: Option<Address>,
    pub tx_nonce: u64,

    pub value: U256,
    pub gas_limit: U256,
    pub gas_price: U256,

    // (block_timestamp, block_number)
    pub block_params: (U256, U256),
    /// Steps executed in the transaction
    pub steps_executed: u64,

    pub tx_exit_status: Option<(
        crate::account::transaction_tree::Status,
        solana_program::keccak::Hash,
    )>,
    // fields for layout_version >= 1
}

impl TrxView for PlainData {
    fn hash(&self) -> [u8; 32] {
        self.tx_hash
    }

    fn gas_price(&self) -> U256 {
        self.gas_price
    }

    fn chain_id(&self) -> Option<u64> {
        self.chain_id
    }

    fn is_scheduled_tx(&self) -> bool {
        self.tree_account.is_some()
    }

    fn nonce(&self) -> u64 {
        self.tx_nonce
    }

    fn gas_limit(&self) -> U256 {
        self.gas_limit
    }

    fn target(&self) -> Option<Address> {
        self.tx_target
    }

    fn value(&self) -> U256 {
        self.value
    }
}

impl PlainData {
    fn layout_version() -> usize {
        0
    }

    pub fn cancel(&self, program_id: &Pubkey, account: &AccountInfo) -> Result<()> {
        // Clear an executor and set the result as canceled
        self.finalize_impl(program_id, TAG_SCHEDULED_STATE_CANCELLED, account)
    }

    fn finalize_impl(
        &self,
        program_id: &Pubkey,
        scheduled_transition_tag: u8,
        account: &AccountInfo,
    ) -> Result<()> {
        let tag = super::tag(program_id, account)?;
        if tag != TAG_STATE {
            return Err(Error::AccountInvalidTag(*account.key, tag));
        }

        if self.tree_account.is_some() {
            debug_print!(
                "Pre-finalize State {} into {} for scheduled transaction",
                account.key,
                scheduled_transition_tag
            );
            // Change the tag, leave all the data unchanged.
            super::set_tag(
                program_id,
                account,
                scheduled_transition_tag,
                Header::VERSION,
            )?;
        } else {
            debug_print!("Finalize State {}", account.key);
            StateFinalizedAccount::make(program_id, self, account)?;
        }

        Ok(())
    }

    pub fn finish_scheduled_tx(&self, program_id: &Pubkey, account: &AccountInfo) -> Result<()> {
        let tag = super::tag(program_id, account)?;
        let is_finalized = tag == TAG_SCHEDULED_STATE_FINALIZED;
        let is_canceled = tag == TAG_SCHEDULED_STATE_CANCELLED;
        if !(is_finalized || is_canceled) {
            return Err(Error::StorageAccountInvalidTag(*account.key, tag));
        }

        debug_print!("Finalize State {} for scheduled transaction", account.key);
        StateFinalizedAccount::make(program_id, self, account)?;

        Ok(())
    }
}

#[repr(C)]
pub struct Root {
    pub plain_data: PlainData,
    /// Stored revision
    revisions: TreeMap<Pubkey, AccountRevision>,
    /// Accounts that been read during the transaction    
    pub touched_accounts: TreeMap<Pubkey, u64>,
    /// State of `execute_external_instruction` at the Solana call interruption breakpoint
    /// None if no Solana call interruption occurs
    pub interrupted_state: Option<InterruptedState>,

    pub executor_state: RefCell<Option<ExecutorStateData>>,
    pub machine_state: RefCell<Option<Machine<crate::evm::tracing::NoopEventListener>>>,
    //pub alloc : SolanaAllocator
}

// to be sure that solana and x86 size/alignment match
const_assert_eq!(std::mem::align_of::<PlainData>(), 0x8);
const_assert_eq!(std::mem::size_of::<PlainData>(), 0x1A0);
const_assert_eq!(std::mem::offset_of!(Root, revisions), 0x1A0);

impl AccountHeader for Header {
    const VERSION: u8 = 2;
}

pub struct StateAccount<'local, 'sol> {
    account: &'local AccountInfo<'sol>,
    root_ref: RefMut<'local, Root>,
    trx_rlp: &'local [u8],

    tag: u8,
}

type StateAccountCoreApiView = (
    PlainData,
    Vec<Pubkey>,
    Vec<u8>, //tx rlp
);

impl PlainData {
    fn use_gas(&mut self, amount: U256) -> Result<U256> {
        if amount == U256::ZERO {
            return Ok(U256::ZERO);
        }

        let total_gas_used = self.gas_used.saturating_add(amount);
        let gas_limit = self.gas_limit;

        if total_gas_used > gas_limit {
            return Err(Error::OutOfGas(gas_limit, total_gas_used));
        }

        self.gas_used = total_gas_used;

        amount
            .checked_mul(self.gas_price)
            .ok_or(Error::IntegerOverflow)
    }

    #[must_use]
    pub fn gas_available(&self) -> U256 {
        self.gas_limit.saturating_sub(self.gas_used)
    }

    /// Use available gas and return it to the caller.
    /// It's caller's responsibility to mint the unused gas tokens to the appropriate recipient.
    pub fn materialize_unused_gas(&mut self) -> Result<U256> {
        let unused_gas = self.gas_available();
        let gas_fee_tokens = self.use_gas(unused_gas)?;

        Ok(gas_fee_tokens)
    }

    pub fn consume_gas(
        &mut self,
        amount: U256,
        receiver: Option<OperatorBalanceAccount>,
    ) -> Result<()> {
        let tokens = self.use_gas(amount)?;

        if tokens == U256::ZERO {
            return Ok(());
        }

        let mut operator_balance = receiver.ok_or(Error::OperatorBalanceMissing)?;

        let trx_chain_id = self.chain_id.unwrap_or(DEFAULT_CHAIN_ID);
        if operator_balance.chain_id() != trx_chain_id {
            return Err(Error::OperatorBalanceInvalidChainId);
        }

        operator_balance.mint(tokens)
    }

    pub fn refund_unused_gas(&mut self, origin: &mut BalanceAccount) -> Result<()> {
        let trx_chain_id = self.chain_id.unwrap_or(DEFAULT_CHAIN_ID);

        assert!(origin.chain_id() == trx_chain_id);
        assert!(origin.address() == self.origin);

        let total_refund = self.materialize_unused_gas()?;

        origin.mint(total_refund)
    }
}

enum RestoreResult<'local, 'sol> {
    State(StateAccount<'local, 'sol>),
    NeedReallocate {
        trx_rlp: &'local [u8],
        header: Box<PlainData>,
    },
}

impl<'local, 'sol> StateAccount<'local, 'sol> {
    #[must_use]
    pub fn into_account(self) -> &'local AccountInfo<'sol> {
        self.account
    }

    fn validate_tag(account_key: &Pubkey, tag: u8) -> Result<()> {
        if tag == TAG_STATE
            || tag == TAG_SCHEDULED_STATE_FINALIZED
            || tag == TAG_SCHEDULED_STATE_CANCELLED
        {
            Ok(())
        } else {
            Err(Error::StorageAccountInvalidTag(*account_key, tag))
        }
    }

    #[must_use]
    pub fn trx_rlp(&self) -> &[u8] {
        self.trx_rlp
    }

    pub fn recover_tx_rlp(
        program_id: &Pubkey,
        account: &'local AccountInfo<'sol>,
    ) -> Result<&'local [u8]> {
        let tag = super::tag(program_id, account)?;
        Self::validate_tag(account.key, tag)?;

        let data_ptr = {
            let data_ref = account.try_borrow_mut_data()?;
            data_ref.as_ptr()
        };

        let (tx_ptr, tx_len) = {
            let header_ref = super::header::<Header>(account);
            (
                unsafe { data_ptr.offset(header_ref.serialized_tx.start.try_into()?) },
                header_ref.serialized_tx.end - header_ref.serialized_tx.start,
            )
        };

        Ok(unsafe { &*slice_from_raw_parts(tx_ptr, tx_len) })
    }

    pub fn recover_plain_header(
        program_id: &Pubkey,
        account: &'local AccountInfo<'sol>,
    ) -> Result<PlainData> {
        let tag = super::tag(program_id, account)?;
        Self::validate_tag(account.key, tag)?;

        let root_offset = super::header::<Header>(account).root_offset;

        let mut plain = PlainData::default();
        {
            let plain_ref = &mut plain;
            let dataref = account.try_borrow_data()?;
            let dataslice: &[u8] = &dataref.as_ref()[root_offset..][..size_of::<PlainData>()];
            unsafe {
                std::slice::from_raw_parts_mut(
                    std::ptr::from_mut::<PlainData>(plain_ref).cast::<u8>(),
                    dataslice.len(),
                )
                .copy_from_slice(dataslice);
            }
        }
        if plain.layout_version != PlainData::layout_version() {
            // clear or default corresponding fields
        }

        Ok(plain)
    }

    // allocator should have provided a properly aligned pointer
    #[allow(clippy::cast_ptr_alignment)]
    fn from_account(
        program_id: &Pubkey,
        account: &'local AccountInfo<'sol>,
    ) -> Result<RestoreResult<'local, 'sol>> {
        let tag = super::tag(program_id, account)?;
        Self::validate_tag(account.key, tag)?;

        let (offset, need_restart) = {
            let header = super::header::<Header>(account);
            (
                header.root_offset,
                header.version_signature != Header::valid_version_signature(),
            )
        };

        let trx_rlp = Self::recover_tx_rlp(program_id, account)?;

        if need_restart {
            return Ok(RestoreResult::NeedReallocate {
                trx_rlp,
                header: Box::new(Self::recover_plain_header(program_id, account)?),
            });
        }

        let mem: RefMut<&mut [u8]> = account.try_borrow_mut_data()?;

        Ok(RestoreResult::State(Self {
            account,
            root_ref: RefMut::map(mem, |data| unsafe {
                &mut *(data
                    .as_mut_ptr()
                    .offset(offset.try_into().unwrap())
                    .cast::<Root>())
            }),
            trx_rlp,
            tag,
        }))
    }

    #[allow(clippy::cast_sign_loss)]
    pub fn new(
        program_id: &Pubkey,
        info: &'local AccountInfo<'sol>,
        accounts: &AccountsDB<'sol>,
        origin: Address,
        transaction: &Transaction,
        transaction_rlp: &[u8],
        tree_account: Option<Pubkey>,
    ) -> Result<Self> {
        let (info, owner) = match super::tag(program_id, info)? {
            TAG_HOLDER => {
                let holder = Holder::from_account(program_id, info)?;
                holder.validate_owner(accounts.operator())?;
                let owner = holder.owner();
                (holder.into_account(), owner)
            }
            TAG_STATE_FINALIZED => {
                let finalized = StateFinalizedAccount::from_account(program_id, info)?;
                finalized.validate_owner(accounts.operator())?;
                finalized.validate_trx(transaction)?;
                let owner = finalized.owner();
                (finalized.into_account(), owner)
            }
            tag => return Err(Error::StorageAccountInvalidTag(*info.key, tag)),
        };

        assert!(
            !(transaction.is_scheduled_tx() ^ tree_account.is_some()),
            "Tree account should be present iff it's a scheduled transaction."
        );

        super::set_tag(program_id, info, TAG_STATE, Header::VERSION)?;

        Self::init_header(
            info,
            origin,
            transaction,
            transaction_rlp,
            tree_account,
            owner,
        )
    }

    #[allow(clippy::cast_sign_loss)]
    fn init_header(
        info: &'local AccountInfo<'sol>,
        origin: Address,
        transaction: &Transaction,
        transaction_rlp: &[u8],
        tree_account: Option<Pubkey>,
        owner: Pubkey,
    ) -> Result<Self> {
        let root = boxx(Root {
            plain_data: PlainData {
                layout_version: PlainData::layout_version(),
                owner,
                origin,
                tree_account,
                gas_used: U256::ZERO,
                tx_hash: transaction.hash(),
                chain_id: transaction.chain_id(),
                tx_target: transaction.target(),
                tx_nonce: transaction.nonce(),
                value: transaction.value(),
                gas_limit: transaction.gas_limit(),
                gas_price: transaction.gas_price(),
                block_params: (U256::ZERO, U256::ZERO),
                steps_executed: 0_u64,
                tx_exit_status: None,
            },
            revisions: TreeMap::new(),
            touched_accounts: TreeMap::new(),
            interrupted_state: None,
            executor_state: None.into(),
            machine_state: None.into(),
        });

        let tx_rlp = transaction_rlp.to_vector();
        let (ptr, len, _) = tx_rlp.into_raw_parts();

        Ok(Self {
            account: info,
            root_ref: RefMut::map(info.try_borrow_mut_data()?, |data| {
                let account_data_ptr = data.as_ptr();
                {
                    // Set header
                    let header = super::header_mut_from_slice::<Header>(data);
                    header.version_signature = Header::valid_version_signature();
                    header.root_offset =
                        unsafe { addr_of!(*root).cast::<u8>().offset_from(account_data_ptr) }
                            as usize;

                    let start = unsafe { ptr.offset_from(account_data_ptr) } as usize;
                    let end = start + len;
                    header.serialized_tx = std::ops::Range::<usize> { start, end };
                }

                unsafe { &mut *Boxx::into_raw(root) }
            }),
            trx_rlp: unsafe { &*slice_from_raw_parts(ptr, len) },
            tag: TAG_STATE,
        })
    }

    pub fn restore(
        program_id: &Pubkey,
        info: &'local AccountInfo<'sol>,
        accounts: &AccountsDB<'sol>,
    ) -> Result<(Self, AccountsStatus, Option<Transaction>)> {
        let mut state = match Self::from_account(program_id, info)? {
            RestoreResult::State(state) => state,
            RestoreResult::NeedReallocate { trx_rlp, header } => {
                let trx_rlp = trx_rlp.to_vec();
                Holder::init_holder_heap(program_id, info, 0)?;
                let transaction = Transaction::parse_from_rlp(trx_rlp.as_slice(), None)?;
                let mut state = Self::init_header(
                    info,
                    header.origin,
                    &transaction,
                    trx_rlp.as_slice(),
                    header.tree_account,
                    header.owner,
                )?;
                state.root_ref_mut().plain_data.gas_used = header.gas_used;
                return Ok((state, AccountsStatus::NeedRestart, Some(transaction)));
            }
        };

        let mut status = state.validate_revisions(program_id, accounts);
        if status == AccountsStatus::Ok {
            status = state.validate_timestamps(program_id, accounts);
        }

        if status == AccountsStatus::NeedRestart {
            // reset all accounts revisions
            state.root_ref.revisions.clear();
            state.root_ref.touched_accounts.clear();
            state.set_interrupted_state(None);
        }

        Ok((state, status, None))
    }

    fn validate_revisions(&self, program_id: &Pubkey, accounts: &AccountsDB) -> AccountsStatus {
        let touched_accounts = self
            .root_ref
            .touched_accounts
            .iter()
            .filter_map(|(key, counter)| if counter >= &2 { Some(key) } else { None });

        for pubkey in touched_accounts {
            let account = accounts.get(pubkey);

            let account_revision = AccountRevision::new(program_id, account);
            let stored_revision = &self.root_ref.revisions[pubkey];

            if stored_revision != &account_revision {
                log_data(&[b"INVALID_REVISION", pubkey.as_ref()]);
                return AccountsStatus::NeedRestart;
            }
        }

        AccountsStatus::Ok
    }

    pub fn finalize_step(&mut self) {
        let BlockParams { number, timestamp } =
            self.executor_state().as_ref().unwrap().block_params;
        self.root_ref.plain_data.block_params = (timestamp, number);

        let tx_exit_status = self
            .executor_state_ref()
            .exit_status
            .as_ref()
            .and_then(TransactionTree::prepare_exit_status);
        self.root_ref.plain_data.tx_exit_status = tx_exit_status;
    }

    fn validate_timestamps(&self, program_id: &Pubkey, accounts: &AccountsDB) -> AccountsStatus {
        let state = self.root_ref.executor_state.borrow();
        let executor_state = state.as_ref().unwrap();
        let state_block_number: u64 = executor_state.block_params.number.as_u64();

        let timestamped_contracts = executor_state.timestamped_contracts.borrow();
        for address in timestamped_contracts.keys() {
            let (pubkey, _) = address.find_solana_address(program_id);
            let account = accounts.get(&pubkey).clone();
            let Ok(contract) = ContractAccount::from_account(program_id, account) else {
                continue;
            };

            if contract.timestamp_used_at() > state_block_number {
                log_data(&[b"INVALID_REVISION", pubkey.as_ref()]);
                return AccountsStatus::NeedRestart;
            }
        }

        AccountsStatus::Ok
    }

    #[must_use]
    pub fn account_key(&self) -> &Pubkey {
        self.account.key
    }

    pub fn finalize(self, program_id: &Pubkey) -> Result<()> {
        self.finalize_impl(program_id, TAG_SCHEDULED_STATE_FINALIZED)
    }

    fn finalize_impl(self, program_id: &Pubkey, scheduled_transition_tag: u8) -> Result<()> {
        if self.tag != TAG_STATE {
            return Err(Error::AccountInvalidTag(*self.account.key, self.tag));
        }

        if self.has_tree_account() {
            debug_print!(
                "Pre-finalize State {} into {} for scheduled transaction",
                self.account.key,
                scheduled_transition_tag
            );
            std::mem::drop(self.root_ref);
            // Change the tag, leave all the data unchanged.
            super::set_tag(
                program_id,
                self.account,
                scheduled_transition_tag,
                Header::VERSION,
            )?;
        } else {
            debug_print!("Finalize State {}", self.account.key);
            StateFinalizedAccount::convert_from_state(program_id, self)?;
        }

        Ok(())
    }

    pub fn update_touched_accounts(
        &mut self,
        program_id: &Pubkey,
        accounts: &AccountsDB,
    ) -> Result<()> {
        let root = self.root_ref_mut();
        for (key, counter) in &*root
            .executor_state
            .borrow()
            .as_ref()
            .unwrap()
            .touched_accounts
            .borrow()
        {
            root.touched_accounts.update_or_insert(*key, counter, |v| {
                v.checked_add(*counter).ok_or(Error::IntegerOverflow)
            })?;
        }

        let touched_accounts = &root.touched_accounts;
        let revisions = &mut root.revisions;

        for (key, _) in touched_accounts {
            let account = accounts.get(key);
            revisions.insert_with_if_not_exists(*key, || AccountRevision::new(program_id, account));
        }

        Ok(())
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Pubkey> {
        self.root_ref.revisions.keys()
    }

    #[must_use]
    pub fn owner(&self) -> Pubkey {
        self.root_ref.plain_data.owner
    }

    #[must_use]
    pub fn trx_origin(&self) -> Address {
        self.root_ref.plain_data.origin
    }

    #[must_use]
    pub fn tree_account(&self) -> Option<Pubkey> {
        self.root_ref.plain_data.tree_account
    }

    fn has_tree_account(&self) -> bool {
        self.root_ref.plain_data.tree_account.is_some()
    }

    #[must_use]
    pub fn trx_chain_id(&self, backend: &impl AccountStorage) -> u64 {
        self.root_ref
            .plain_data
            .chain_id
            .unwrap_or_else(|| backend.default_chain_id())
    }

    #[must_use]
    pub fn gas_used(&self) -> U256 {
        self.root_ref.plain_data.gas_used
    }

    #[must_use]
    pub fn gas_available(&self) -> U256 {
        self.root_ref
            .plain_data
            .gas_limit
            .saturating_sub(self.gas_used())
    }

    pub fn consume_gas(
        &mut self,
        amount: U256,
        receiver: Option<OperatorBalanceAccount>,
    ) -> Result<()> {
        self.root_ref.plain_data.consume_gas(amount, receiver)
    }

    pub fn refund_unused_gas(&mut self, origin: &mut BalanceAccount) -> Result<()> {
        self.root_ref.plain_data.refund_unused_gas(origin)
    }

    /// Use available gas and return it to the caller.
    /// It's caller's responsibility to mint the unused gas tokens to the appropriate recipient.
    pub fn materialize_unused_gas(&mut self) -> Result<U256> {
        self.root_ref.plain_data.materialize_unused_gas()
    }

    #[must_use]
    pub fn steps_executed(&self) -> u64 {
        self.root_ref.plain_data.steps_executed
    }

    pub fn reset_steps_executed(&mut self) {
        self.root_ref.plain_data.steps_executed = 0;
    }

    pub fn increment_steps_executed(&mut self, steps: u64) -> Result<()> {
        self.root_ref.plain_data.steps_executed = self
            .root_ref
            .plain_data
            .steps_executed
            .checked_add(steps)
            .ok_or(Error::IntegerOverflow)?;

        Ok(())
    }

    #[must_use]
    pub fn interrupted_state(&self) -> Option<&InterruptedState> {
        self.root_ref.interrupted_state.as_ref()
    }

    pub fn set_interrupted_state(&mut self, state: Option<InterruptedState>) {
        self.root_ref.interrupted_state = state;
    }

    #[must_use]
    pub fn trx(&self) -> &impl TrxView {
        &self.root_ref.plain_data
    }
}

// Implementation of functional to save/restore persistent state of iterative transactions.
impl StateAccount<'_, '_> {
    #[must_use]
    pub fn executor_state(&self) -> Ref<Option<ExecutorStateData>> {
        self.root_ref.executor_state.borrow()
    }

    #[must_use]
    pub fn executor_state_ref(&self) -> Ref<ExecutorStateData> {
        Ref::map(self.root_ref.executor_state.borrow(), |x| {
            x.as_ref().unwrap()
        })
    }

    #[must_use]
    pub fn executor_state_mut(&self) -> RefMut<Option<ExecutorStateData>> {
        self.root_ref.executor_state.borrow_mut()
    }

    #[must_use]
    pub fn executor_state_mut_ref(&self) -> RefMut<ExecutorStateData> {
        RefMut::map(self.root_ref.executor_state.borrow_mut(), |x| {
            x.as_mut().unwrap()
        })
    }

    #[must_use]
    pub fn evm(&self) -> Ref<Option<Machine<crate::evm::tracing::NoopEventListener>>> {
        self.root_ref.machine_state.borrow()
    }

    #[must_use]
    pub fn evm_ref(&self) -> Ref<Machine<crate::evm::tracing::NoopEventListener>> {
        Ref::map(self.root_ref.machine_state.borrow(), |x| {
            x.as_ref().unwrap()
        })
    }

    #[must_use]
    pub fn evm_mut(&self) -> RefMut<Option<Machine<crate::evm::tracing::NoopEventListener>>> {
        self.root_ref.machine_state.borrow_mut()
    }

    #[must_use]
    pub fn evm_mut_ref(&self) -> RefMut<Machine<crate::evm::tracing::NoopEventListener>> {
        RefMut::map(self.root_ref.machine_state.borrow_mut(), |x| {
            x.as_mut().unwrap()
        })
    }

    #[must_use]
    pub fn root_ref(&self) -> &Root {
        &self.root_ref
    }

    #[must_use]
    pub fn root_ref_mut(&mut self) -> &mut Root {
        &mut self.root_ref
    }
}

impl<'local, 'sol> StateAccount<'local, 'sol> {
    /// Implementation to squeeze bits of information from the state account.
    /// N.B.
    /// 1. `StateAccount` contains objects and pointers allocated by the state account allocator, so reading
    ///     objects inside requires jumping on the offset (between the real account address as allocated by the
    ///     current allocator) and "intended" address of the first account as provided by the Solana runtime.
    /// 2. `addr_of!` and `read_unaligned` is heavily used to facilitate the reading of fields by raw pointers.
    /// 3. There are upcasts from *const u8 to *const T, but since T was allocated by the allocator previously,
    ///     it has the correct alignment and the upcast is sound.
    #[allow(clippy::cast_ptr_alignment)]
    pub fn get_state_account_view(
        program_id: &Pubkey,
        account: &'local AccountInfo<'sol>,
    ) -> Result<StateAccountCoreApiView> {
        Self::validate_tag(account.key, super::tag(program_id, account)?)?;

        let account_data_ptr = account.try_borrow_data()?.as_ptr();

        let (tx_start, tx_end, root_offset) = {
            let header = super::header::<Header>(account);
            (
                header.serialized_tx.start,
                header.serialized_tx.end,
                header.root_offset,
            )
        };

        let tx_rlp: Vec<u8> = account.try_borrow_data()?.as_ref()[..tx_end][tx_start..].to_vec();

        let root_ptr: *const Root =
            unsafe { account_data_ptr.add(root_offset).cast::<Root>().cast() };

        let plain = Self::recover_plain_header(program_id, account)?;

        if plain.layout_version == PlainData::layout_version() {
            let memory_space_delta = {
                account_data_ptr as isize
                    - isize::try_from(crate::allocator::STATE_ACCOUNT_DATA_ADDRESS)?
            };
            let accounts = unsafe {
                // Hereby we read the TreeMap and rely on the fact that under the hood it's a Vector<(Pubkey, AccountRevision)>.
                // In case the structure changes, it also requires adjustments.
                read_vec::<(Pubkey, AccountRevision)>(
                    addr_of!((*root_ptr).revisions).cast::<usize>(),
                    memory_space_delta,
                )
                .iter()
                .map(|(key, _)| *key)
                .collect()
            };
            Ok((plain, accounts, tx_rlp))
        } else {
            // we don't have a reliable way to reconstruct revisions
            Ok((plain, Vec::<Pubkey>::new(), tx_rlp))
        }
    }
}
