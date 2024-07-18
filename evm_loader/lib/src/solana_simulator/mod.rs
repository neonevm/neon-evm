use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use log::debug;
use solana_accounts_db::transaction_results::TransactionExecutionDetails;
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_loader_v4_program::create_program_runtime_environment_v2;
use solana_program_runtime::loaded_programs::{
    LoadedProgram, LoadedProgramsForTxBatch, ProgramRuntimeEnvironments,
};
use solana_program_runtime::log_collector::LogCollector;
use solana_program_runtime::message_processor::MessageProcessor;
use solana_program_runtime::sysvar_cache::SysvarCache;
use solana_program_runtime::timings::ExecuteTimings;
use solana_runtime::accounts::construct_instructions_account;
use solana_runtime::builtins::BUILTINS;
use solana_runtime::{bank::TransactionSimulationResult, runtime_config::RuntimeConfig};
use solana_sdk::account::{
    create_account_shared_data_with_fields, AccountSharedData, ReadableAccount,
    DUMMY_INHERITABLE_ACCOUNT_FIELDS, PROGRAM_OWNERS,
};
use solana_sdk::address_lookup_table::error::AddressLookupError;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::clock::Clock;
use solana_sdk::feature_set::FeatureSet;
use solana_sdk::fee_calculator::DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE;
use solana_sdk::message::v0::{LoadedAddresses, MessageAddressTableLookup};
use solana_sdk::message::{AddressLoader, AddressLoaderError};
use solana_sdk::rent::Rent;
use solana_sdk::transaction_context::{IndexOfAccount, TransactionContext};
use solana_sdk::{
    account::Account,
    address_lookup_table, bpf_loader_upgradeable,
    hash::Hash,
    pubkey::Pubkey,
    signature::Keypair,
    sysvar::{Sysvar, SysvarId},
    transaction::{SanitizedTransaction, VersionedTransaction},
};

pub use error::Error;
pub use utils::SyncState;

use crate::rpc::Rpc;

mod error;
mod utils;

#[derive(Debug)]
pub struct SolanaSimulator {
    runtime_config: RuntimeConfig,
    feature_set: Arc<FeatureSet>,
    accounts_db: HashMap<Pubkey, AccountSharedData>,
    sysvar_cache: SysvarCache,
    payer: Keypair,
}

impl SolanaSimulator {
    pub async fn new(rpc: &impl Rpc) -> Result<Self, Error> {
        Self::new_with_config(rpc, RuntimeConfig::default(), SyncState::Yes).await
    }

    pub async fn new_without_sync(rpc: &impl Rpc) -> Result<Self, Error> {
        Self::new_with_config(rpc, RuntimeConfig::default(), SyncState::No).await
    }

    pub async fn new_with_config(
        _rpc: &impl Rpc,
        runtime_config: RuntimeConfig,
        sync_state: SyncState,
    ) -> Result<Self, Error> {
        if sync_state == SyncState::Yes {
            // utils::sync_sysvar_accounts(rpc, &bank).await?;
        }

        let feature_set = Arc::new(FeatureSet::all_enabled());

        let mut sysvar_cache = SysvarCache::default();

        sysvar_cache.set_rent(Rent::default());
        sysvar_cache.set_clock(Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: 0,
        });

        Ok(Self {
            runtime_config,
            feature_set,
            accounts_db: HashMap::new(),
            sysvar_cache,
            payer: Keypair::new(),
        })
    }

    pub async fn sync_accounts(&mut self, rpc: &impl Rpc, keys: &[Pubkey]) -> Result<(), Error> {
        let mut storable_accounts: Vec<(&Pubkey, &Account)> = vec![];

        let mut programdata_keys = vec![];

        let mut accounts = rpc.get_multiple_accounts(keys).await?;
        for (key, account) in keys.iter().zip(&mut accounts) {
            let Some(account) = account else {
                continue;
            };

            if account.executable && bpf_loader_upgradeable::check_id(&account.owner) {
                let programdata_address = utils::program_data_address(account)?;
                debug!(
                    "program_data_account: program={key} programdata=address{programdata_address}"
                );
                programdata_keys.push(programdata_address);
            }

            if account.owner == address_lookup_table::program::id() {
                utils::reset_alt_slot(account).map_err(|_| Error::InvalidALT)?;
            }

            storable_accounts.push((key, account));
        }

        let mut programdata_accounts = rpc.get_multiple_accounts(&programdata_keys).await?;
        for (key, account) in programdata_keys.iter().zip(&mut programdata_accounts) {
            let Some(account) = account else {
                continue;
            };

            debug!("program_data_account: key={key} account={account:?}");
            utils::reset_program_data_slot(account)?;
            storable_accounts.push((key, account));
        }

        self.set_multiple_accounts(&storable_accounts);

        Ok(())
    }

    pub const fn payer(&self) -> &Keypair {
        &self.payer
    }

    pub fn blockhash(&self) -> Hash {
        Hash::new_unique()
    }

    pub fn slot(&self) -> Result<u64, Error> {
        let clock = self.sysvar_cache.get_clock()?;
        Ok(clock.slot)
    }

    fn replace_sysvar_account<S>(&mut self, sysvar: &S)
    where
        S: Sysvar + SysvarId,
    {
        let old_account = self.accounts_db.get(&S::id());
        let inherit = old_account
            .map(|a| (a.lamports(), a.rent_epoch()))
            .unwrap_or(DUMMY_INHERITABLE_ACCOUNT_FIELDS);

        let account = create_account_shared_data_with_fields(sysvar, inherit);
        self.accounts_db.insert(S::id(), account);
    }

    pub fn set_clock(&mut self, clock: Clock) {
        self.replace_sysvar_account(&clock);
        self.sysvar_cache.set_clock(clock);
    }

    pub fn set_account(&mut self, pubkey: Pubkey, account: Account) {
        self.accounts_db.insert(pubkey, account.into());
    }

    pub fn set_multiple_accounts(&mut self, accounts: &[(&Pubkey, &Account)]) {
        for (pubkey, account) in accounts {
            self.accounts_db
                .insert(**pubkey, AccountSharedData::from((*account).clone()));
        }
    }

    pub fn get_shared_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts_db.get(pubkey).cloned()
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.get_shared_account(pubkey).map(Account::from)
    }

    pub fn sanitize_transaction(
        &self,
        tx: VersionedTransaction,
        verify: bool,
    ) -> Result<SanitizedTransaction, Error> {
        let sanitized_tx = {
            let size = bincode::serialized_size(&tx)?;
            if verify && (size > solana_sdk::packet::PACKET_DATA_SIZE as u64) {
                return Err(solana_sdk::transaction::TransactionError::SanitizeFailure.into());
            }

            let message_hash = if verify {
                tx.verify_and_hash_message()?
            } else {
                tx.message.hash()
            };

            SanitizedTransaction::try_create(tx, message_hash, None, self)
        }?;

        if verify {
            sanitized_tx.verify_precompiles(&self.feature_set)?;
        }

        Ok(sanitized_tx)
    }

    pub fn process_transaction(
        &mut self,
        blockhash: Hash,
        tx: SanitizedTransaction,
    ) -> Result<TransactionExecutionDetails, Error> {
        let mut transaction_accounts = Vec::new();
        for key in tx.message().account_keys().iter() {
            let account = if solana_sdk::sysvar::instructions::check_id(key) {
                construct_instructions_account(tx.message())
            } else {
                self.accounts_db.get(key).cloned().unwrap_or_default()
            };
            transaction_accounts.push((*key, account));
        }
        //
        let builtins_start_index = transaction_accounts.len();
        let program_indices: Vec<Vec<IndexOfAccount>> = tx
            .message()
            .instructions()
            .iter()
            .map(|instruction| {
                let mut account_indices: Vec<IndexOfAccount> = Vec::with_capacity(2);

                let program_index = instruction.program_id_index as usize;
                let (program_id, program_account) = &transaction_accounts[program_index];

                if solana_sdk::native_loader::check_id(program_id) {
                    return account_indices;
                }

                account_indices.insert(0, program_index as IndexOfAccount);

                let owner = program_account.owner();
                if solana_sdk::native_loader::check_id(owner) {
                    return account_indices;
                }

                if let Some(owner_index) = transaction_accounts[builtins_start_index..]
                    .iter()
                    .position(|(key, _)| key == owner)
                {
                    let owner_index = owner_index + builtins_start_index;
                    account_indices.insert(0, owner_index as IndexOfAccount);
                } else {
                    let _builtin = BUILTINS
                        .iter()
                        .find(|builtin| builtin.program_id == *owner)
                        .unwrap();

                    let owner_account =
                        AccountSharedData::new(100, 100, &solana_sdk::native_loader::id());
                    transaction_accounts.push((*owner, owner_account));

                    let owner_index = transaction_accounts.len() - 1;
                    account_indices.insert(0, owner_index as IndexOfAccount);
                }

                account_indices
            })
            .collect();
        //
        let compute_budget = self.runtime_config.compute_budget.unwrap_or_default();
        let rent: Arc<Rent> = self.sysvar_cache.get_rent()?;
        let clock: Arc<Clock> = self.sysvar_cache.get_clock()?;
        //
        let mut transaction_context = TransactionContext::new(
            transaction_accounts,
            *rent,
            compute_budget.max_invoke_stack_height,
            compute_budget.max_instruction_trace_length,
        );
        //
        let program_runtime_environments = ProgramRuntimeEnvironments {
            program_runtime_v1: Arc::new(create_program_runtime_environment_v1(
                &self.feature_set,
                &compute_budget,
                true,
                true,
            )?),
            program_runtime_v2: Arc::new(create_program_runtime_environment_v2(
                &compute_budget,
                true,
            )),
        };
        let mut loaded_programs = LoadedProgramsForTxBatch::new(
            clock.slot,
            program_runtime_environments,
            None,
            clock.epoch,
        );
        tx.message()
            .account_keys()
            .iter()
            .for_each(|key| match loaded_programs.find(key) {
                Some(_) => {}
                None => {
                    let account = self.accounts_db.get(key).cloned().unwrap_or_default();
                    if PROGRAM_OWNERS.iter().any(|owner| account.owner() == owner) {
                        // use Bank::load_program_from_bytes
                        loaded_programs.replenish(*key, Arc::new(LoadedProgram::new().unwrap()));
                    }
                }
            });
        for builtin in BUILTINS {
            // create_loadable_account_with_fields
            let program = LoadedProgram::new_builtin(0, builtin.name.len(), builtin.entrypoint);
            loaded_programs.replenish(builtin.program_id, Arc::new(program));
        }
        //
        let mut modified_programs = LoadedProgramsForTxBatch::new(
            clock.slot,
            loaded_programs.environments.clone(),
            loaded_programs.upcoming_environments.clone(),
            loaded_programs.latest_root_epoch,
        );
        //
        let log_limit = self.runtime_config.log_messages_bytes_limit;
        let log_collector = LogCollector::new_ref_with_limit(log_limit);
        //
        let mut timings = ExecuteTimings::default();
        let mut executed_units = 0u64;
        //
        let _result = MessageProcessor::process_message(
            tx.message(),
            &program_indices,
            &mut transaction_context,
            Some(Rc::clone(&log_collector)),
            &loaded_programs,
            &mut modified_programs,
            Arc::clone(&self.feature_set),
            compute_budget,
            &mut timings,
            &self.sysvar_cache,
            blockhash,
            DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE / 2,
            &mut executed_units,
        )?;

        todo!();
    }

    pub fn simulate_transaction(
        &self,
        _tx: VersionedTransaction,
    ) -> Result<TransactionSimulationResult, Error> {
        todo!()
    }

    pub fn simulate_legacy_transaction(
        &self,
        tx: solana_sdk::transaction::Transaction,
    ) -> Result<TransactionSimulationResult, Error> {
        let versioned_transaction = VersionedTransaction::from(tx);
        self.simulate_transaction(versioned_transaction)
    }
}

impl AddressLoader for &SolanaSimulator {
    fn load_addresses(
        self,
        lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        let loaded_addresses = lookups
            .iter()
            .map(|address_table_lookup| {
                let table_account = self
                    .get_shared_account(&address_table_lookup.account_key)
                    .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

                if table_account.owner() != &address_lookup_table::program::id() {
                    return Err(AddressLookupError::InvalidAccountOwner);
                }

                let current_slot = self
                    .slot()
                    .map_err(|_| AddressLookupError::LookupTableAccountNotFound)?;

                let slot_hashes = self
                    .sysvar_cache
                    .get_slot_hashes()
                    .map_err(|_| AddressLookupError::LookupTableAccountNotFound)?;

                let lookup_table = AddressLookupTable::deserialize(table_account.data())
                    .map_err(|_| AddressLookupError::InvalidAccountData)?;

                Ok(LoadedAddresses {
                    writable: lookup_table.lookup(
                        current_slot,
                        &address_table_lookup.writable_indexes,
                        &slot_hashes,
                    )?,
                    readonly: lookup_table.lookup(
                        current_slot,
                        &address_table_lookup.readonly_indexes,
                        &slot_hashes,
                    )?,
                })
            })
            .collect::<Result<_, AddressLookupError>>()?;

        Ok(loaded_addresses)
    }
}
