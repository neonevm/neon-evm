use std::collections::HashMap;

use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::instruction::Instruction;
use solana_program::program::{invoke_signed_unchecked, invoke_unchecked};
use solana_program::system_program;

use crate::account::{AllocateResult, BalanceAccount, ContractAccount, StorageCell};
use crate::account_storage::ProgramAccountStorage;
use crate::config::{
    ACCOUNT_SEED_VERSION, PAYMENT_TO_TREASURE, STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
};
use crate::error::Result;
use crate::executor::Action;
use crate::types::{Address, Vector};

impl<'a> ProgramAccountStorage<'a> {
    pub fn transfer_treasury_payment(&mut self) -> Result<()> {
        let system = self.accounts.system();
        let treasury = self.accounts.treasury();
        let operator = self.accounts.operator();

        system.transfer(operator, treasury, PAYMENT_TO_TREASURE)?;

        Ok(())
    }

    pub fn transfer_gas_payment(
        &mut self,
        origin: Address,
        chain_id: u64,
        value: U256,
    ) -> Result<()> {
        let (pubkey, _) = origin.find_balance_address(&crate::ID, chain_id);

        let source = self.accounts.get(&pubkey).clone();
        let mut source = BalanceAccount::from_account(&crate::ID, source)?;

        let target = self.accounts.operator_balance();

        source.transfer(target, value)
    }

    pub fn allocate(&mut self, actions: &[Action]) -> Result<AllocateResult> {
        let mut total_result = AllocateResult::Ready;

        for action in actions {
            if let Action::EvmSetCode { address, code, .. } = action {
                let result = ContractAccount::allocate(
                    *address,
                    code,
                    &self.rent,
                    &self.accounts,
                    Some(&self.keys),
                )?;

                if result == AllocateResult::NeedMore {
                    total_result = AllocateResult::NeedMore;
                }
            }
        }

        Ok(total_result)
    }

    pub fn apply_state_change(&mut self, actions: Vector<Action>) -> Result<()> {
        debug_print!("Applies begin");

        let mut storage = HashMap::with_capacity(16);

        for action in actions {
            match action {
                Action::Transfer {
                    source,
                    target,
                    chain_id,
                    value,
                } => {
                    let mut source = self.balance_account(source, chain_id)?;
                    let mut target = self.create_balance_account(target, chain_id)?;
                    source.transfer(&mut target, value)?;
                }
                Action::Burn {
                    source,
                    chain_id,
                    value,
                } => {
                    let mut account = self.create_balance_account(source, chain_id)?;
                    account.burn(value)?;
                }
                Action::EvmSetStorage {
                    address,
                    index,
                    value,
                } => {
                    storage
                        .entry(address)
                        .or_insert_with(|| HashMap::with_capacity(64))
                        .insert(index, value);
                }
                Action::EvmIncrementNonce { address, chain_id } => {
                    let mut account = self.create_balance_account(address, chain_id)?;
                    account.increment_nonce()?;
                }
                Action::EvmSetCode {
                    address,
                    chain_id,
                    code,
                } => {
                    ContractAccount::init(
                        address,
                        chain_id,
                        0,
                        &code,
                        &self.accounts,
                        Some(&self.keys),
                    )?;
                }
                Action::EvmSelfDestruct { address: _ } => {
                    // EIP-6780: SELFDESTRUCT only in the same transaction
                    // do nothing, balance was already transfered
                }
                Action::ExternalInstruction {
                    program_id,
                    accounts,
                    data,
                    seeds,
                    ..
                } => {
                    let seeds: Vec<&[u8]> = seeds.iter().map(|seed| &seed[..]).collect();

                    let mut accounts_info = Vec::with_capacity(accounts.len() + 1);

                    let program = self.accounts.get(&program_id).clone();
                    accounts_info.push(program);

                    for meta in &accounts {
                        let account: AccountInfo<'a> =
                            if meta.pubkey == self.accounts.operator_key() {
                                self.accounts.operator_info().clone()
                            } else {
                                self.accounts.get(&meta.pubkey).clone()
                            };
                        accounts_info.push(account);
                    }

                    let instruction = Instruction {
                        program_id,
                        accounts: accounts.to_vec(),
                        data: data.to_vec(),
                    };

                    if !seeds.is_empty() {
                        invoke_signed_unchecked(&instruction, &accounts_info, &[&seeds])?;
                    } else {
                        invoke_unchecked(&instruction, &accounts_info)?;
                    }
                }
            }
        }

        self.apply_storage(storage)?;

        debug_print!("Applies done");

        Ok(())
    }

    fn apply_storage(&mut self, storage: HashMap<Address, HashMap<U256, [u8; 32]>>) -> Result<()> {
        const STATIC_STORAGE_LIMIT: U256 = U256::new(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u128);

        for (address, storage) in storage {
            let mut contract: Option<ContractAccount> = None;

            let mut infinite_values: HashMap<U256, HashMap<u8, [u8; 32]>> =
                HashMap::with_capacity(storage.len());

            for (index, value) in storage {
                if index < STATIC_STORAGE_LIMIT {
                    let contract = contract.get_or_insert_with(|| {
                        self.contract_account(address)
                            .expect("contract already created")
                    });

                    // Static Storage - Write into contract account
                    let index: usize = index.as_usize();
                    contract.set_storage_value(index, &value);
                } else {
                    // Infinite Storage - Write into separate account
                    let subindex = (index & 0xFF).as_u8();
                    let index = index & !U256::new(0xFF);

                    infinite_values
                        .entry(index)
                        .or_insert_with(|| HashMap::with_capacity(32))
                        .insert(subindex, value);
                }
            }

            if let Some(mut contract) = contract {
                contract.increment_revision(&self.rent, &self.accounts)?;
            }

            for (index, values) in infinite_values {
                let cell_address = self.keys.storage_cell_address(&crate::ID, address, index);

                let account = self.accounts.get(cell_address.pubkey());

                if system_program::check_id(account.owner) {
                    let (_, bump) = self.keys.contract_with_bump_seed(&crate::ID, address);
                    let sign: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], address.as_bytes(), &[bump]];

                    let len = values.len();
                    let mut storage =
                        StorageCell::create(cell_address, len, &self.accounts, sign, &self.rent)?;
                    let mut cells = storage.cells_mut();

                    assert_eq!(cells.len(), len);
                    for (cell, (subindex, value)) in cells.iter_mut().zip(values) {
                        cell.subindex = subindex;
                        cell.value = value;
                    }
                } else {
                    let mut storage = StorageCell::from_account(&crate::ID, account.clone())?;
                    for (subindex, value) in values {
                        storage.update(subindex, &value)?;
                    }

                    storage.sync_lamports(&self.rent, &self.accounts)?;
                    storage.increment_revision(&self.rent, &self.accounts)?;
                };
            }
        }

        Ok(())
    }
}
