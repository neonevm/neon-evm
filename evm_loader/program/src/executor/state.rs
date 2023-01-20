use std::cell::RefCell;
use std::collections::BTreeMap;

use ethnum::U256;
use serde::Serialize;
use serde::de::DeserializeSeed;
use bincode::Options;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;

use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::error::{Error, Result};
use crate::evm::{ExitStatus, Context};
use crate::evm::database::Database;
use crate::types::Address;

use super::{OwnedAccountInfo, OwnedAccountInfoPartial};
use super::action::Action;
use super::cache::{Cache};


/// Represents the state of executor abstracted away from a self.backend.
/// UPDATE `serialize/deserialize` WHEN THIS STRUCTURE CHANGES
pub struct ExecutorState<'a, B: AccountStorage> {
    pub backend: &'a B,
    cache: RefCell<Cache>,
    actions: Vec<Action>,
    stack: Vec<usize>,
    exit_status: Option<ExitStatus>,
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
            actions: Vec::with_capacity(64),
            stack: Vec::with_capacity(16),
            exit_status: None,
        }
    }

    pub fn into_actions(self) -> Vec<Action> {
        self.actions
    }

    pub fn exit_status(&self) -> Option<&ExitStatus> {
        self.exit_status.as_ref()
    }

    pub fn set_exit_status(&mut self, status: ExitStatus) {
        self.exit_status = Some(status);
    }

    pub fn call_depth(&self) -> usize {
        self.stack.len()
    }

    pub fn withdraw_neons(&mut self, source: Address, value: U256) {
        let withdraw = Action::NeonWithdraw { source, value };
        self.actions.push(withdraw);
    }

    pub fn queue_external_instruction(
        &mut self, 
        instruction: Instruction, 
        seeds: Vec<Vec<u8>>,
        allocate: usize,
    ) {
        let action = Action::ExternalInstruction {
            program_id: instruction.program_id,
            data: instruction.data,
            accounts: instruction.accounts,
            seeds,
            allocate
        };
        
        self.actions.push(action);
    }

    pub fn external_account(&self, address: Pubkey) -> Result<OwnedAccountInfo> {
        let mut cache = self.cache.borrow_mut();

        let metas = self.actions.iter()
            .filter_map(|a| if let Action::ExternalInstruction { accounts, .. } = a { Some(accounts) } else { None })
            .flatten()
            .collect::<Vec<_>>();

        if !metas.iter().any(|m| (m.pubkey == address) && m.is_writable) {
            return Ok(cache.get_account_or_insert(address, self.backend).clone())
        }

        let mut accounts = metas.into_iter()
            .map(|m| (m.pubkey, cache.get_account_or_insert(m.pubkey, self.backend).clone()))
            .collect::<BTreeMap<Pubkey, OwnedAccountInfo>>();

        for action in &self.actions {
            if let Action::ExternalInstruction { program_id, data, accounts: meta, .. } = action {
                match program_id {
                    program_id if solana_program::system_program::check_id(program_id) => {
                        crate::external_programs::system::emulate(data, meta, &mut accounts)?;
                    },
                    program_id if spl_token::check_id(program_id) => {
                        crate::external_programs::spl_token::emulate(data, meta, &mut accounts)?;
                    },
                    program_id if spl_associated_token_account::check_id(program_id) => {
                        crate::external_programs::spl_associated_token::emulate(data, meta, &mut accounts)?;
                    },
                    program_id if mpl_token_metadata::check_id(program_id) => {
                        crate::external_programs::metaplex::emulate(data, meta, &mut accounts)?;
                    },
                    _ => {
                        return Err(Error::Custom(format!("Unknown external program: {program_id}")));
                    }
                }
            }
        }

        Ok(accounts[&address].clone())
    }

    pub fn external_account_partial_cache(&mut self, address: Pubkey, offset: usize, len: usize) -> Result<()> {
        if (len == 0) || (len > 8*1024) {
            return Err(Error::Custom("Account cache: invalid data len".into()));
        }
        
        if let Some(account) = self.backend.clone_solana_account_partial(&address, offset, len) {
            let mut cache = self.cache.borrow_mut();
            cache.solana_accounts_partial.insert(address, account);
    
            Ok(())
        } else {
            Err(Error::Custom("Account cache: invalid data offset".into()))
        }
    }

    pub fn external_account_partial(&self, address: Pubkey) -> Result<OwnedAccountInfoPartial> {
        let cache = self.cache.borrow();
        cache.solana_accounts_partial.get(&address)
            .cloned()
            .ok_or_else(|| Error::Custom(format!("Account cache: account {} is not cached", address)))
    }
}


impl<'a, B: AccountStorage> Database for ExecutorState<'a, B> {
    fn chain_id(&self) -> U256 {
        let chain_id = self.backend.chain_id();
        U256::from(chain_id)
    }

    fn nonce(&self, from_address: &Address) -> Result<u64> {
        let mut nonce = self.backend.nonce(from_address);

        for action in &self.actions {
            if let Action::EvmIncrementNonce { address } = action {
                if from_address == address {
                    nonce += 1;
                }
            }
        }

        Ok(nonce)
    }

    fn increment_nonce(&mut self, address: Address) -> Result<()> {
        let increment = Action::EvmIncrementNonce { address };
        self.actions.push(increment);

        Ok(())
    }

    fn balance(&self, from_address: &Address) -> Result<U256> {
        let mut balance = self.backend.balance(from_address);

        for action in &self.actions {
            match action {
                Action::NeonTransfer { source, target, value } => {
                    if from_address == source {
                        balance -= value;
                    }

                    if from_address == target {
                        balance += value;
                    }
                }
                Action::NeonWithdraw { source, value } => {
                    if from_address == source {
                        balance -= value;
                    }
                }
                _ => {}
            }
        }

        Ok(balance)
    }

    fn transfer(&mut self, source: Address, target: Address, value: U256) -> Result<()> {
        if value == U256::ZERO {
            return Ok(())
        }

        if source == target {
            return Ok(())
        }

        if self.balance(&source)? < value {
            return Err(Error::InsufficientBalanceForTransfer(source, value));
        }

        let transfer = Action::NeonTransfer { source, target, value };
        self.actions.push(transfer);

        Ok(())
    }

    fn code_size(&self, from_address: &Address) -> Result<usize> {
        if self.is_precompile_extension(from_address) {
            return Ok(1); // This is required in order to make a normal call to an extension contract
        }

        for action in &self.actions {
            if let Action::EvmSetCode { address, code } = action {
                if from_address == address {
                    return Ok(code.len());
                }
            }
        }

       Ok(self.backend.code_size(from_address))
    }

    fn code_hash(&self, from_address: &Address) -> Result<[u8; 32]> {
        use solana_program::keccak::hash;

        for action in &self.actions {
            if let Action::EvmSetCode { address, code } = action {
                if from_address == address {
                    return Ok(hash(code).to_bytes());
                }
            }
        }

        Ok(self.backend.code_hash(from_address))
    }

    fn code(&self, from_address: &Address) -> Result<Vec<u8>> {
        for action in &self.actions {
            if let Action::EvmSetCode { address, code } = action {
                if from_address == address {
                    return Ok(code.clone())
                }
            }
        }

        Ok(self.backend.code(from_address))
    }

    fn set_code(&mut self, address: Address, code: Vec<u8>) -> Result<()> {
        if code.starts_with(&[0xEF]) {
            // https://eips.ethereum.org/EIPS/eip-3541
            return Err(Error::EVMObjectFormatNotSupported(address));
        }

        if code.len() > 0x6000 {
            // https://eips.ethereum.org/EIPS/eip-170
            return Err(Error::ContractCodeSizeLimit(address, code.len()));
        }

        let set_code = Action::EvmSetCode { address, code };
        self.actions.push(set_code);

        Ok(())
    }

    fn selfdestruct(&mut self, address: Address) -> Result<()> {
        let suicide = Action::EvmSelfDestruct { address };
        self.actions.push(suicide);

        Ok(())
    }

    fn storage(&self, from_address: &Address, from_index: &U256) -> Result<[u8; 32]> {
        for action in self.actions.iter().rev() {
            if let Action::EvmSetStorage { address, index, value } = action {
                if (from_address == address) && (from_index == index) {
                    return Ok(*value);
                }
            }
        }

        Ok(self.backend.storage(from_address, from_index))
    }

    fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()> {
        let set_storage = Action::EvmSetStorage { address, index, value };
        self.actions.push(set_storage);

        Ok(())
    }

    fn block_hash(&self, number: U256) -> Result<[u8; 32]> {
        let origin_block = self.cache.borrow().block_number;
        let current_block = self.backend.block_number();
        let offset = current_block.saturating_sub(origin_block);

        let number = number.saturating_add(offset);
        let block_hash = self.backend.block_hash(number);

        Ok(block_hash)
    }

    fn block_number(&self) -> Result<U256> {
        let cache = self.cache.borrow();
        Ok(cache.block_number)
    }

    fn block_timestamp(&self) -> Result<U256> {
        let cache = self.cache.borrow();
        Ok(cache.block_timestamp)
    }

    fn log(&mut self, address: Address, topics: &[[u8; 32]], data: &[u8]) -> Result<()> {
        let log = Action::EvmLog {
            address,
            topics: topics.to_vec(),
            data: data.to_vec()
        };
        self.actions.push(log);

        Ok(())
    }

    fn snapshot(&mut self) -> Result<()> {
        self.stack.push(self.actions.len());

        Ok(())
    }

    fn revert_snapshot(&mut self) -> Result<()> {
        let actions_len = self.stack.pop().unwrap_or(0);
        self.actions.truncate(actions_len);

        Ok(())
    }

    fn commit_snapshot(&mut self) -> Result<()> {
        self.stack.pop();

        Ok(())
    }

    fn precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        data: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>> {
        self.call_precompile_extension(context, address, data, is_static)
    }
}


impl<'a> Serialize for ExecutorState<'_, ProgramAccountStorage<'a>> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&self.cache)?;
        seq.serialize_element(&self.actions)?;
        seq.serialize_element(&self.stack)?;
        seq.serialize_element(&self.exit_status)?;

        seq.end()
    }
}

impl<'de, 'a> DeserializeSeed<'de> for &'de ProgramAccountStorage<'a> {
    type Value = ExecutorState<'de, ProgramAccountStorage<'a>>;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        struct SeqVisitor;

        impl<'de> serde::de::Visitor<'de> for SeqVisitor {
            type Value = (RefCell<Cache>, Vec<Action>, Vec<usize>, Option<ExitStatus>);

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Iterative Executor State")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
                where
                    A: serde::de::SeqAccess<'de>
            {
                let cache = seq.next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let actions = seq.next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let stack = seq.next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                let exit_status = seq.next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?;

                Ok((cache, actions, stack, exit_status))
            }
        }

        let (cache, actions, stack, exit_status) 
            = deserializer.deserialize_seq(SeqVisitor)?;

        Ok(ExecutorState { backend: self, cache, actions, stack, exit_status })
    }
}

impl<'de, 'a> ExecutorState<'de, ProgramAccountStorage<'a>> {
    pub fn serialize_into<W>(&self, writer: &mut W) -> Result<()> 
        where W: std::io::Write
    {
        let bincode = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes();

        bincode.serialize_into(writer, &self)
            .map_err(Error::from)
    }

    pub fn deserialize_from(buffer: &mut &[u8], backend: &'de ProgramAccountStorage<'a>) -> Result<Self> 
    {
        let bincode = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes();

        bincode.deserialize_from_seed(backend, buffer)
            .map_err(Error::from)
    }
}