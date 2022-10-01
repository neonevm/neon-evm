use std::collections::BTreeMap;
use std::convert::TryInto;

use evm::{H160, U256};
use solana_program::entrypoint::{MAX_PERMITTED_DATA_INCREASE, ProgramResult};
use solana_program::instruction::Instruction;
use solana_program::program::{invoke, invoke_signed_unchecked};
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::system_instruction;
use solana_program::sysvar::Sysvar;

use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumStorage, Operator, program};
use crate::account_storage::{AccountOperation, AccountsOperations, AccountsReadiness, AccountStorage, ProgramAccountStorage};
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;
use crate::executor::{AccountMeta, Action};

impl<'a> ProgramAccountStorage<'a> {
    pub fn transfer_gas_payment(
        &mut self,
        origin: H160,
        mut operator: EthereumAccount<'a>,
        value: U256,
    ) -> ProgramResult {
        let origin_balance = self.balance(&origin);
        if origin_balance < value {
            self.transfer_gas_payment(origin, operator, origin_balance)?;
            return Err!(ProgramError::InsufficientFunds; "Account {} - insufficient funds", origin);
        }

        if operator.address == origin {
            return Ok(())
        }

        if self.ethereum_accounts.contains_key(&operator.address) {
            self.transfer_neon_tokens(&origin, &operator.address, value)?;
            core::mem::drop(operator);
        } else {
            let origin_account = self.ethereum_account_mut(&origin);
            // balance checked above

            origin_account.balance -= value;
            operator.balance += value;
        }

        Ok(())
    }

    pub fn apply_state_change(
        &mut self,
        neon_program: &program::Neon<'a>,
        system_program: &program::System<'a>,
        operator: &Operator<'a>,
        actions: Vec<Action>,
        accounts_operations: AccountsOperations,
    ) -> Result<AccountsReadiness, ProgramError> {
        debug_print!("Applies begin");

        if self.process_accounts_operations(
            system_program,
            neon_program,
            operator,
            accounts_operations,
        )? == AccountsReadiness::NeedMoreReallocations {
            debug_print!("Applies postponed: need to reallocate accounts in the next transaction(s)");
            return Ok(AccountsReadiness::NeedMoreReallocations);
        }

        let mut storage: BTreeMap<H160, Vec<(U256, U256)>> = BTreeMap::new();

        for action in actions {
            match action {
                Action::NeonTransfer { source, target, value } => {
                    self.transfer_neon_tokens(&source, &target, value)?;
                },
                Action::NeonWithdraw { source, value } => {
                    let account = self.ethereum_account_mut(&source);
                    if account.balance < value {
                        return Err!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, required = {}", source, value)?;
                    }

                    account.balance -= value;
                },
                Action::EvmLog { address, topics, data } => {
                    neon_program.on_event(address, &topics, &data)?;
                },
                Action::EvmSetStorage { address, key, value } => {
                    storage.entry(address).or_default().push((key, value));
                },
                Action::EvmIncrementNonce { address } => {
                    let account = self.ethereum_account_mut(&address);
                    if account.trx_count == u64::MAX {
                        return Err!(ProgramError::InvalidAccountData; "Account {} - nonce overflow", account.address);
                    }

                    account.trx_count += 1;
                },
                Action::EvmSetCode { address, code, valids } => {
                    self.deploy_contract(address, &code, &valids)?;
                },
                Action::EvmSelfDestruct { address } => {
                    storage.remove(&address);

                    self.delete_account(address)?;
                },
                Action::ExternalInstruction { program_id, instruction, accounts, seeds } => {
                    let seeds: Vec<&[u8]> = seeds.iter().map(|seed| &seed[..]).collect();
                    let accounts: Vec<_> = accounts.into_iter().map(AccountMeta::into_solana_meta).collect();

                    let mut accounts_info = Vec::with_capacity(accounts.len() + 1);

                    accounts_info.push(self.solana_accounts[&program_id].clone());
                    for meta in &accounts {
                        accounts_info.push(self.solana_accounts[&meta.pubkey].clone());
                    }

                    let instruction = Instruction { program_id, accounts, data: instruction };
                    invoke_signed_unchecked(&instruction, &accounts_info, &[&seeds[..]])?;
                }
            }
        }

        for (address, storage) in storage {
            for (key, value) in storage {
                if key < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                    let index: usize = key.as_usize() * 32;
                    let account = self.ethereum_account(&address)
                        .expect("Account not found");
                    let contract = account.contract_data()
                        .expect("Contract expected");
                    value.to_big_endian(&mut contract.storage()[index..index+32]);
                } else {
                    self.update_storage_infinite(address, key, value, operator, system_program)?;
                }
            }
        }

        debug_print!("Applies done");

        Ok(AccountsReadiness::Ready)
    }

    fn process_accounts_operations(
        &mut self,
        system_program: &program::System<'a>,
        neon_program: &program::Neon<'a>,
        operator: &Operator<'a>,
        accounts_operations: AccountsOperations,
    ) -> Result<AccountsReadiness, ProgramError> {
        let mut accounts_readiness = AccountsReadiness::Ready;
        for (address, operation) in accounts_operations {
            let (solana_address, bump_seed) = self.calc_solana_address(&address);
            let solana_account = self.solana_account(&solana_address)
                .ok_or_else(||
                    E!(
                        ProgramError::UninitializedAccount;
                        "Account {} - corresponding Solana account was not provided",
                        address
                    )
                )?;
            match operation {
                AccountOperation::Create { space } => {
                    debug_print!("Creating account (space = {})", space);
                    EthereumAccount::create_account(
                        system_program,
                        neon_program.key,
                        operator,
                        address,
                        solana_account,
                        bump_seed,
                        MAX_PERMITTED_DATA_INCREASE.min(space),
                    )?;

                    self.add_ether_account(neon_program.key, solana_account)?;

                    if space > MAX_PERMITTED_DATA_INCREASE {
                        accounts_readiness = AccountsReadiness::NeedMoreReallocations;
                    }
                }

                AccountOperation::Resize { from, to } => {
                    debug_print!("Resizing account (from = {}, to = {})", from, to);

                    assert_eq!(solana_account.owner, self.program_id);

                    let rent = Rent::get()?;
                    let lamports_needed = rent.minimum_balance(
                        to.min(from.saturating_add(MAX_PERMITTED_DATA_INCREASE)),
                    );
                    let lamports_current = solana_account.lamports();
                    if lamports_current < lamports_needed {
                        invoke(
                            &system_instruction::transfer(
                                operator.key,
                                solana_account.key,
                                lamports_needed.saturating_sub(lamports_current),
                            ),
                            &[
                                (*operator.info).clone(),
                                solana_account.clone(),
                                (*system_program).clone(),
                            ],
                        )?;
                    }

                    let max_possible_space_per_instruction = to
                        .min(from + MAX_PERMITTED_DATA_INCREASE);
                    solana_account.realloc(max_possible_space_per_instruction, false)?;

                    if max_possible_space_per_instruction < to {
                        accounts_readiness = AccountsReadiness::NeedMoreReallocations;
                    }
                }
            };
        }

        Ok(accounts_readiness)
    }

    /// Delete all data in the account.
    fn delete_account(&mut self, address: H160) -> ProgramResult {
        let account = self.ethereum_account_mut(&address);

        assert_eq!(account.balance, U256::zero()); // balance should be moved by executor
        account.trx_count = 0;
        account.generation = account.generation.checked_add(1)
            .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - generation overflow", address))?;

        if let Some(contract) = account.contract_data() {
            contract.extension_borrow_mut().fill(0);
        }

        account.code_size = 0;

        Ok(())
    }

    fn deploy_contract(
        &mut self,
        address: H160,
        code: &[u8],
        valids: &[u8],
    ) -> ProgramResult {
        let account = self.ethereum_accounts.get_mut(&address)
            .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address))?;

        assert_eq!(
            account.code_size,
            0,
            "Contract already deployed to address {} (code_size = {})!",
            account.address,
            account.code_size,
        );

        let space_needed = EthereumAccount::space_needed(code.len());
        let space_actual = account.info.data_len();
        assert!(
            space_actual >= space_needed,
            "Not enough space for account deployment at address {} \
                (code size: {}, space needed: {}, actual space: {})",
            account.address,
            code.len(),
            space_needed,
            space_actual,
        );

        account.code_size = code.len()
            .try_into()
            .expect("code.len() never exceeds u32::max");

        let contract = account.contract_data()
            .expect("Contract data must be available at this point");

        contract.code().copy_from_slice(code);
        contract.valids().copy_from_slice(valids);

        Ok(())
    }

    pub fn update_storage_infinite(
        &mut self,
        address: H160,
        index: U256,
        value: U256,
        operator: &Operator<'a>,
        system_program: &program::System<'a>,
    ) -> ProgramResult {
        #[allow(clippy::cast_possible_truncation)]
        let subindex = (index & U256::from(0xFF)).as_u64() as u8;
        let index = index & !U256::from(0xFF);

        let (solana_address, bump_seed) = self.get_storage_address(&address, &index);
        let account = self.solana_accounts.get(&solana_address)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - storage account not found", solana_address))?;

        if account.owner == self.program_id {
            let mut storage = EthereumStorage::from_account(self.program_id, account)?;
            storage.set(subindex, value, operator, system_program)?;

            return Ok(());
        }

        if solana_program::system_program::check_id(account.owner) {
            if value.is_zero() {
                return Ok(());
            }

            let generation_bytes = self.generation(&address).to_le_bytes();

            let mut index_bytes = [0_u8; 32];
            index.to_little_endian(&mut index_bytes);
    
            let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ContractStorage", address.as_bytes(), &generation_bytes, &index_bytes, &[bump_seed]];
            system_program.create_pda_account(self.program_id, operator, account, seeds, EthereumStorage::SIZE)?;

            let mut storage = EthereumStorage::init(account, crate::account::ether_storage::Data {})?;
            storage.set(subindex, value, operator, system_program)?;

            return Ok(())
        }

        Err!(ProgramError::InvalidAccountData; "Account {} - expected system or program owned", solana_address)
    }


    fn transfer_neon_tokens(&mut self, source: &H160, target: &H160, value: U256) -> ProgramResult {
        debug_print!("Transfer {} NEONs from {} to {}", value, source, target);

        if source == target {
            return Ok(())
        }

        if !self.ethereum_accounts.contains_key(source) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expect initialized", source);
        }
        if !self.ethereum_accounts.contains_key(target) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expect initialized", source);
        }

        if self.balance(source) < value {
            return Err!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, required = {}", source, value)
        }

        self.ethereum_account_mut(source).balance -= value;
        self.ethereum_account_mut(target).balance += value;

        Ok(())
    }
    
}
