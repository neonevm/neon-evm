use std::cell::RefCell;
use std::collections::BTreeMap;

use ethnum::{AsU256, U256};
use maybe_async::maybe_async;
use mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;

use crate::account_storage::AccountStorage;
use crate::error::{Error, Result};
use crate::evm::database::Database;
use crate::evm::{Context, ExitStatus};
use crate::types::Address;

use super::action::Action;
use super::cache::{cache_get_or_insert_account, cache_get_or_insert_balance, Cache};
use super::OwnedAccountInfo;

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
    pub fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize> {
        let mut cursor = std::io::Cursor::new(buffer);

        let value = (&self.cache, &self.actions, &self.stack, &self.exit_status);
        bincode::serialize_into(&mut cursor, &value)?;

        cursor.position().try_into().map_err(Error::from)
    }

    pub fn deserialize_from(buffer: &[u8], backend: &'a B) -> Result<Self> {
        let (cache, actions, stack, exit_status) = bincode::deserialize(buffer)?;
        Ok(Self {
            backend,
            cache,
            actions,
            stack,
            exit_status,
        })
    }

    #[must_use]
    pub fn new(backend: &'a B) -> Self {
        let cache = Cache {
            solana_accounts: BTreeMap::new(),
            native_balances: BTreeMap::new(),
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
        assert!(self.stack.is_empty());

        crate::executor::action::filter_selfdestruct(self.actions)
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

    pub fn burn(&mut self, source: Address, chain_id: u64, value: U256) {
        let burn = Action::Burn {
            source,
            chain_id,
            value,
        };
        self.actions.push(burn);
    }

    pub fn queue_external_instruction(
        &mut self,
        instruction: Instruction,
        seeds: Vec<Vec<u8>>,
        fee: u64,
    ) {
        let action = Action::ExternalInstruction {
            program_id: instruction.program_id,
            data: instruction.data,
            accounts: instruction.accounts,
            seeds,
            fee,
        };

        self.actions.push(action);
    }

    #[maybe_async]
    pub async fn external_account(&self, address: Pubkey) -> Result<OwnedAccountInfo> {
        let metas = self
            .actions
            .iter()
            .filter_map(|a| {
                if let Action::ExternalInstruction { accounts, .. } = a {
                    Some(accounts)
                } else {
                    None
                }
            })
            .flatten()
            .collect::<Vec<_>>();

        if !metas.iter().any(|m| (m.pubkey == address) && m.is_writable) {
            let account = cache_get_or_insert_account(&self.cache, address, self.backend).await;
            return Ok(account);
        }

        let mut accounts = BTreeMap::<Pubkey, OwnedAccountInfo>::new();

        for m in metas {
            let account = cache_get_or_insert_account(&self.cache, m.pubkey, self.backend).await;
            accounts.insert(account.key, account);
        }

        for action in &self.actions {
            if let Action::ExternalInstruction {
                program_id,
                data,
                accounts: meta,
                ..
            } = action
            {
                match program_id {
                    program_id if solana_program::system_program::check_id(program_id) => {
                        crate::external_programs::system::emulate(data, meta, &mut accounts)?;
                    }
                    program_id if spl_token::check_id(program_id) => {
                        crate::external_programs::spl_token::emulate(data, meta, &mut accounts)?;
                    }
                    program_id if spl_associated_token_account::check_id(program_id) => {
                        crate::external_programs::spl_associated_token::emulate(
                            data,
                            meta,
                            &mut accounts,
                            self.rent(),
                        )?;
                    }
                    program_id if &MPL_TOKEN_METADATA_ID == program_id => {
                        crate::external_programs::metaplex::emulate(
                            data,
                            meta,
                            &mut accounts,
                            self.rent(),
                        )?;
                    }
                    _ => {
                        return Err(Error::Custom(format!(
                            "Unknown external program: {program_id}"
                        )));
                    }
                }
            }
        }

        Ok(accounts[&address].clone())
    }
}

#[maybe_async(?Send)]
impl<'a, B: AccountStorage> Database for ExecutorState<'a, B> {
    async fn nonce(&self, from_address: Address, from_chain_id: u64) -> Result<u64> {
        let mut nonce = self.backend.nonce(from_address, from_chain_id).await;
        let mut increment = 0_u64;

        for action in &self.actions {
            if let Action::EvmIncrementNonce { address, chain_id } = action {
                if (&from_address == address) && (&from_chain_id == chain_id) {
                    increment += 1;
                }
            }
        }

        nonce = nonce.checked_add(increment).ok_or(Error::IntegerOverflow)?;

        Ok(nonce)
    }

    fn increment_nonce(&mut self, address: Address, chain_id: u64) -> Result<()> {
        let increment = Action::EvmIncrementNonce { address, chain_id };
        self.actions.push(increment);

        Ok(())
    }

    async fn balance(&self, from_address: Address, from_chain_id: u64) -> Result<U256> {
        let cache_key = (from_address, from_chain_id);
        let mut balance = cache_get_or_insert_balance(&self.cache, cache_key, self.backend).await;

        for action in &self.actions {
            match action {
                Action::Transfer {
                    source,
                    target,
                    chain_id,
                    value,
                } if (&from_chain_id == chain_id) => {
                    if &from_address == source {
                        balance = balance.checked_sub(*value).ok_or(Error::IntegerOverflow)?;
                    }

                    if &from_address == target {
                        balance = balance.checked_add(*value).ok_or(Error::IntegerOverflow)?;
                    }
                }
                Action::Burn {
                    source,
                    chain_id,
                    value,
                } if (&from_chain_id == chain_id) && (&from_address == source) => {
                    balance = balance.checked_sub(*value).ok_or(Error::IntegerOverflow)?;
                }
                _ => {}
            }
        }

        Ok(balance)
    }

    async fn transfer(
        &mut self,
        source: Address,
        target: Address,
        chain_id: u64,
        value: U256,
    ) -> Result<()> {
        if value == U256::ZERO {
            return Ok(());
        }

        let target_chain_id = self.contract_chain_id(target).await.unwrap_or(chain_id);

        if (self.code_size(target).await? > 0) && (target_chain_id != chain_id) {
            return Err(Error::InvalidTransferToken(source, chain_id));
        }

        if source == target {
            return Ok(());
        }

        if self.balance(source, chain_id).await? < value {
            return Err(Error::InsufficientBalance(source, chain_id, value));
        }

        let transfer = Action::Transfer {
            source,
            target,
            chain_id,
            value,
        };
        self.actions.push(transfer);

        Ok(())
    }

    async fn code_size(&self, from_address: Address) -> Result<usize> {
        if self.is_precompile_extension(&from_address) {
            return Ok(1); // This is required in order to make a normal call to an extension contract
        }

        for action in &self.actions {
            if let Action::EvmSetCode { address, code, .. } = action {
                if &from_address == address {
                    return Ok(code.len());
                }
            }
        }

        Ok(self.backend.code_size(from_address).await)
    }

    async fn code(&self, from_address: Address) -> Result<crate::evm::Buffer> {
        for action in &self.actions {
            if let Action::EvmSetCode { address, code, .. } = action {
                if &from_address == address {
                    return Ok(crate::evm::Buffer::from_slice(code));
                }
            }
        }

        Ok(self.backend.code(from_address).await)
    }

    fn set_code(&mut self, address: Address, chain_id: u64, code: Vec<u8>) -> Result<()> {
        if code.starts_with(&[0xEF]) {
            // https://eips.ethereum.org/EIPS/eip-3541
            return Err(Error::EVMObjectFormatNotSupported(address));
        }

        if code.len() > 0x6000 {
            // https://eips.ethereum.org/EIPS/eip-170
            return Err(Error::ContractCodeSizeLimit(address, code.len()));
        }

        let set_code = Action::EvmSetCode {
            address,
            chain_id,
            code,
        };
        self.actions.push(set_code);

        Ok(())
    }

    fn selfdestruct(&mut self, address: Address) -> Result<()> {
        let suicide = Action::EvmSelfDestruct { address };
        self.actions.push(suicide);

        Ok(())
    }

    async fn storage(&self, from_address: Address, from_index: U256) -> Result<[u8; 32]> {
        for action in self.actions.iter().rev() {
            if let Action::EvmSetStorage {
                address,
                index,
                value,
            } = action
            {
                if (&from_address == address) && (&from_index == index) {
                    return Ok(*value);
                }
            }
        }

        Ok(self.backend.storage(from_address, from_index).await)
    }

    fn set_storage(&mut self, address: Address, index: U256, value: [u8; 32]) -> Result<()> {
        let set_storage = Action::EvmSetStorage {
            address,
            index,
            value,
        };
        self.actions.push(set_storage);

        Ok(())
    }

    async fn block_hash(&self, number: U256) -> Result<[u8; 32]> {
        // geth:
        //  - checks the overflow
        //  - converts to u64
        //  - checks on last 256 blocks

        if number >= u64::MAX.as_u256() {
            return Ok(<[u8; 32]>::default());
        }

        let number = number.as_u64();
        let block_slot = self.cache.borrow().block_number.as_u64();
        let lower_block_slot = if block_slot < 257 {
            0
        } else {
            block_slot - 256
        };

        if number >= block_slot || lower_block_slot > number {
            return Ok(<[u8; 32]>::default());
        }

        Ok(self.backend.block_hash(number).await)
    }

    fn block_number(&self) -> Result<U256> {
        let cache = self.cache.borrow();
        Ok(cache.block_number)
    }

    fn block_timestamp(&self) -> Result<U256> {
        let cache = self.cache.borrow();
        Ok(cache.block_timestamp)
    }

    fn rent(&self) -> &Rent {
        self.backend.rent()
    }

    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
        self.backend.return_data()
    }

    async fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&solana_program::account_info::AccountInfo) -> R,
    {
        self.backend.map_solana_account(address, action).await
    }

    fn snapshot(&mut self) {
        self.stack.push(self.actions.len());
    }

    fn revert_snapshot(&mut self) {
        let actions_len = self
            .stack
            .pop()
            .expect("Fatal Error: Inconsistent EVM Call Stack");

        self.actions.truncate(actions_len);

        if self.stack.is_empty() {
            // sanity check
            assert!(self.actions.is_empty());
        }
    }

    fn commit_snapshot(&mut self) {
        self.stack
            .pop()
            .expect("Fatal Error: Inconsistent EVM Call Stack");
    }

    async fn precompile_extension(
        &mut self,
        context: &Context,
        address: &Address,
        data: &[u8],
        is_static: bool,
    ) -> Option<Result<Vec<u8>>> {
        self.call_precompile_extension(context, address, data, is_static)
            .await
    }

    fn default_chain_id(&self) -> u64 {
        self.backend.default_chain_id()
    }

    fn is_valid_chain_id(&self, chain_id: u64) -> bool {
        self.backend.is_valid_chain_id(chain_id)
    }

    async fn contract_chain_id(&self, contract: Address) -> Result<u64> {
        for action in self.actions.iter().rev() {
            if let Action::EvmSetCode {
                address, chain_id, ..
            } = action
            {
                if &contract == address {
                    return Ok(*chain_id);
                }
            }
        }

        self.backend.contract_chain_id(contract).await
    }
}
