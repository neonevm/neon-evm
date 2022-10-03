use std::cell::RefCell;
use std::collections::BTreeMap;

use evm::{H160, U256, H256, ExitError, ExitReason};
use solana_program::instruction::Instruction;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use borsh::{BorshSerialize, BorshDeserialize};

use crate::account_storage::AccountStorage;
use crate::executor::cache::AccountMeta;

use super::{OwnedAccountInfo, OwnedAccountInfoPartial};
use super::action::Action;
use super::cache::Cache;


/// Represents the state of executor abstracted away from a self.backend.
/// UPDATE `serialize/deserialize` WHEN THIS STRUCTURE CHANGES
pub struct ExecutorState<'a, B: AccountStorage> {
    pub backend: &'a B,
    cache: RefCell<Cache>,
    actions: Vec<Action>,
    stack: Vec<usize>,
    is_static: u32,
    exit_result: Option<(Vec<u8>, ExitReason)>,
}

impl<'a, B: AccountStorage> ExecutorState<'a, B> {
    #[must_use]
    pub fn new(backend: &'a B) -> Self {
        let cache = Cache {
            solana_accounts: BTreeMap::new(),
            solana_accounts_partial: BTreeMap::new(),
            block_number: backend.block_number(),
            block_timestamp: backend.block_timestamp(),
        };

        Self {
            backend,
            cache: RefCell::new(cache),
            actions: Vec::new(),
            stack: Vec::new(),
            is_static: 0_u32,
            exit_result: None,
        }
    }

    pub fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.cache.borrow().serialize(writer)?;
        self.actions.serialize(writer)?;
        self.stack.serialize(writer)?;
        self.is_static.serialize(writer)?;
        self.exit_result.serialize(writer)?;

        Ok(())
    }

    pub fn deserialize(buffer: &mut &[u8], backend: &'a B) -> std::io::Result<Self> {
        Ok(Self {
            backend,
            cache: RefCell::new(BorshDeserialize::deserialize(buffer)?),
            actions: BorshDeserialize::deserialize(buffer)?,
            stack: BorshDeserialize::deserialize(buffer)?,
            is_static: BorshDeserialize::deserialize(buffer)?,
            exit_result: BorshDeserialize::deserialize(buffer)?,
        })
    }

    pub fn into_actions(self) -> Vec<Action> {
        self.actions
    }

    /// Creates a snapshot of `ExecutorState` when entering next execution of a call or create.
    pub fn enter(&mut self, is_static: bool) {
        if (self.is_static > 0) || is_static {
            self.is_static += 1;
        }

        self.stack.push(self.actions.len());
    }

    /// Commits the state on exit of call or creation.
    pub fn exit_commit(&mut self) {
        self.stack.pop();

        self.is_static = self.is_static.saturating_sub(1);
    }

    /// Reverts the state on exit of call or creation.
    pub fn exit_revert(&mut self) {
        let actions_len = self.stack.pop().unwrap_or(0);
        self.actions.truncate(actions_len);

        self.is_static = self.is_static.saturating_sub(1);
    }


    /// Increments nonce of an account: increases it by 1.
    pub fn inc_nonce(&mut self, address: H160) {
        let increment = Action::EvmIncrementNonce { address };
        self.actions.push(increment);
    }

    /// Adds or changes a record in the storage of given account.
    pub fn set_storage(&mut self, address: H160, key: U256, value: U256) {
        let set_storage = Action::EvmSetStorage { address, key, value };
        self.actions.push(set_storage);
    }

    /// Adds an Ethereum event log record.
    pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        let log = Action::EvmLog { address, topics, data };
        self.actions.push(log);
    }

    /// Initializes a contract account with it's code and corresponding bit array of valid jumps.
    pub fn set_code(&mut self, address: H160, code: Vec<u8>) {
        let valids = evm::Valids::compute(&code);

        let set_code = Action::EvmSetCode { address, code, valids };
        self.actions.push(set_code);
    }

    /// Marks an account as deleted.
    pub fn set_deleted(&mut self, address: H160) {
        let suicide = Action::EvmSelfDestruct { address };
        self.actions.push(suicide);
    }

    /// Adds a transfer to execute.
    /// # Errors
    /// May return `OutOfFund` if the source has no funds.
    pub fn transfer(&mut self, source: H160, target: H160, value: U256) -> Result<(), ExitError> {
        if value.is_zero() {
            return Ok(())
        }

        if source == target {
            return Ok(())
        }

        if self.balance(&source) < value {
            return Err(ExitError::OutOfFund);
        }

        let transfer = Action::NeonTransfer { source, target, value };
        self.actions.push(transfer);

        Ok(())
    }

    pub fn withdraw(&mut self, source: H160, value: U256) {
        let withdraw = Action::NeonWithdraw { source, value };
        self.actions.push(withdraw);
    }

    pub fn queue_external_instruction(&mut self, instruction: Instruction, seeds: Vec<Vec<u8>>) {
        let action = Action::ExternalInstruction {
            program_id: instruction.program_id,
            instruction: instruction.data,
            accounts: instruction.accounts.into_iter().map(AccountMeta::from_solana_meta).collect(),
            seeds
        };
        
        self.actions.push(action);
    }

    #[must_use]
    pub fn is_static_context(&self) -> bool {
        self.is_static > 0
    }

    #[must_use]
    pub fn call_depth(&self) -> usize {
        self.stack.len()
    }

    #[must_use]
    pub fn balance(&self, from_address: &H160) -> U256 {
        let mut balance = self.backend.balance(from_address);

        for action in &self.actions {
            match action {
                Action::NeonTransfer { source, target, value } => {
                    if from_address == source {
                        balance -= *value;
                    }

                    if from_address == target {
                        balance += *value;
                    }
                }
                Action::NeonWithdraw { source, value } => {
                    if from_address == source {
                        balance -= *value;
                    }
                }
                _ => {}
            }
        }

        balance
    }

    #[must_use]
    pub fn nonce(&self, from_address: &H160) -> U256 {
        let mut nonce = self.backend.nonce(from_address);

        for action in &self.actions {
            match action {
                Action::EvmIncrementNonce { address } => {
                    if from_address == address {
                        nonce += U256::one();
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        nonce = U256::zero();
                    }
                }
                _ => {}
            }
        }

        nonce
    }

    #[must_use]
    pub fn storage(&self, from_address: &H160, from_key: &U256) -> U256 {
        let mut known_storage: Option<U256> = None;

        for action in &self.actions {
            match action {
                Action::EvmSetStorage { address, key, value } => {
                    if (from_address == address) && (from_key == key) {
                        known_storage = Some(*value);
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        known_storage = Some(U256::zero());
                    }
                }
                _ => {}
            }
        }

        known_storage.unwrap_or_else(|| self.backend.storage(from_address, from_key))
    }

    #[must_use]
    pub fn code_size(&self, from_address: &H160) -> U256 {
        let mut code_size = self.backend.code_size(from_address);

        for action in &self.actions {
            match action {
                Action::EvmSetCode { address, code, valids: _ } => {
                    if from_address == address {
                        code_size = code.len();
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        code_size = 0_usize;
                    }
                }
                _ => {}
            }
        }

        U256::from(code_size)
    }

    #[must_use]
    pub fn code_hash(&self, from_address: &H160) -> H256 {
        let mut known_code: Option<&[u8]> = None;

        for action in &self.actions {
            match action {
                Action::EvmSetCode { address, code, valids: _ } => {
                    if from_address == address {
                        known_code = Some(code);
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        known_code = Some(&[]);
                    }
                }
                _ => {}
            }
        }

        known_code.map_or_else(|| self.backend.code_hash(from_address), crate::utils::keccak256_h256)
    }

    #[must_use]
    pub fn code(&self, from_address: &H160) -> Vec<u8> {
        let mut known_code: Option<&[u8]> = None;

        for action in &self.actions {
            match action {
                Action::EvmSetCode { address, code, valids: _ } => {
                    if from_address == address {
                        known_code = Some(code);
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        known_code = Some(&[]);
                    }
                }
                _ => {}
            }
        }

        known_code.map_or_else(|| self.backend.code(from_address), <[u8]>::to_vec)
    }

    #[must_use]
    pub fn valids(&self, from_address: &H160) -> Vec<u8> {
        let mut known_valids: Option<&[u8]> = None;

        for action in &self.actions {
            match action {
                Action::EvmSetCode { address, code: _, valids } => {
                    if from_address == address {
                        known_valids = Some(valids);
                    }
                }
                Action::EvmSelfDestruct { address } => {
                    if from_address == address {
                        known_valids = Some(&[]);
                    }
                }
                _ => {}
            }
        }

        known_valids.map_or_else(|| self.backend.valids(from_address), <[u8]>::to_vec)
    }

    #[must_use]
    pub fn block_hash(&self, number: U256) -> H256 {
        let origin_block = self.cache.borrow().block_number;
        let current_block = self.backend.block_number();
        let offset = current_block.saturating_sub(origin_block);

        let number = number.saturating_add(offset);
        self.backend.block_hash(number)
    }

    #[must_use]
    pub fn block_number(&self) -> U256 {
        self.cache.borrow().block_number
    }

    #[must_use]
    pub fn block_timestamp(&self) -> U256 {
        self.cache.borrow().block_timestamp
    }

    pub fn external_account(&self, address: Pubkey) -> Result<OwnedAccountInfo, ProgramError> {
        let mut cache = self.cache.borrow_mut();


        let metas = self.actions.iter()
            .filter_map(|a| if let Action::ExternalInstruction { accounts, .. } = a { Some(accounts) } else { None })
            .flatten()
            .collect::<Vec<_>>();

        if !metas.iter().any(|m| (m.key == address) && m.is_writable) {
            return Ok(cache.get_account_or_insert(address, self.backend).clone())
        }

        let mut accounts = metas.into_iter()
            .map(|m| (m.key, cache.get_account_or_insert(m.key, self.backend).clone()))
            .collect::<BTreeMap<Pubkey, OwnedAccountInfo>>();

        for action in &self.actions {
            if let Action::ExternalInstruction { program_id, instruction, accounts: meta, .. } = action {
                match program_id {
                    program_id if solana_program::system_program::check_id(program_id) => {
                        crate::external_programs::system::emulate(instruction, meta, &mut accounts)?;
                    },
                    program_id if spl_token::check_id(program_id) => {
                        crate::external_programs::spl_token::emulate(instruction, meta, &mut accounts)?;
                    },
                    program_id if spl_associated_token_account::check_id(program_id) => {
                        crate::external_programs::spl_associated_token::emulate(instruction, meta, &mut accounts)?;
                    },
                    program_id if mpl_token_metadata::check_id(program_id) => {
                        crate::external_programs::metaplex::emulate(instruction, meta, &mut accounts)?;
                    },
                    _ => {
                        return Err!(ProgramError::IncorrectProgramId; "Unknown external program: {}", program_id);
                    }
                }
            }
        }

        Ok(accounts[&address].clone())
    }

    pub fn external_account_partial_cache(&mut self, address: Pubkey, offset: usize, len: usize) -> Result<(), ProgramError> {
        if (len == 0) || (len > 8*1024) {
            return Err!(ProgramError::InvalidArgument; "Account cache: invalid data len");
        }
        
        if let Some(account) = self.backend.clone_solana_account_partial(&address, offset, len) {
            let mut cache = self.cache.borrow_mut();
            cache.solana_accounts_partial.insert(address, account);
    
            Ok(())
        } else {
            Err!(ProgramError::InvalidArgument; "Account cache: invalid data offset")
        }
    }

    pub fn external_account_partial(&self, address: Pubkey) -> Result<OwnedAccountInfoPartial, ProgramError> {
        let cache = self.cache.borrow();
        cache.solana_accounts_partial.get(&address)
            .cloned()
            .ok_or_else(|| E!(ProgramError::NotEnoughAccountKeys; "Account cache: account {} is not cached", address))
    }

    pub fn set_exit_result(&mut self, result: Option<(Vec<u8>, ExitReason)>) {
        self.exit_result = result;
    }

    pub fn exit_result(&self) -> &Option<(Vec<u8>, ExitReason)> {
        &self.exit_result
    }
}
