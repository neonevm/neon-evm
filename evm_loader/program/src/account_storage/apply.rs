use std::cmp::min;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::ops::Deref;
use std::mem::ManuallyDrop;
use evm::{H160, U256};
use solana_program::instruction::Instruction;
use solana_program::{program_error::ProgramError, system_instruction};
use solana_program::entrypoint::{MAX_PERMITTED_DATA_INCREASE, ProgramResult};
use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumStorage, Operator, program};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::executor::{Action, AccountMeta};
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use solana_program::program::{invoke, invoke_signed_unchecked};
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use crate::account::ether_account::ContractExtension;


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
            self.transfer_neon_tokens(origin, operator.address, value)?;
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
    ) -> ProgramResult {

        debug_print!("Applies begin");

        let mut storage: BTreeMap<H160, Vec<(U256, U256)>> = BTreeMap::new();

        for action in actions {
            match action {
                Action::NeonTransfer { source, target, value } => {
                    self.transfer_neon_tokens(source, target, value)?;
                },
                Action::NeonWithdraw { source, value } => {
                    let account = self.ethereum_account_mut(&source);
                    if account.balance < value {
                        return Err!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, required = {}", source, value)?;
                    }

                    account.balance -= value;
                },
                Action::EvmLog { address, topics, data } => {
                    neon_program.on_event(address, topics, data)?;
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
                    self.deploy_contract(address, &code, &valids, system_program, operator)?;
                },
                Action::EvmSelfDestruct { address } => {
                    storage.remove(&address);

                    self.delete_account(address)?;
                },
                Action::ExternalInstruction { program_id, instruction, accounts, seeds } => {
                    let seeds: Vec<&[u8]> = seeds.iter().map(|seed| &seed[..]).collect();
                    let accounts: Vec<_> = accounts.into_iter().map(AccountMeta::into_solana_meta).collect();

                    let mut accounts_info = Vec::with_capacity(accounts.len() + 1);
                    let mut use_account_info = |key| match key {
                        key if key == operator.key => {
                            accounts_info.push(operator.deref().clone());
                        },
                        key if key == system_program.key => {
                            accounts_info.push(system_program.deref().clone());
                        },
                        key => {
                            accounts_info.push(self.solana_accounts[key].clone());
                        }
                    };

                    use_account_info(&program_id);
                    for meta in &accounts {
                        use_account_info(&meta.pubkey);
                    }

                    let instruction = Instruction { program_id, accounts, data: instruction };
                    invoke_signed_unchecked(&instruction, &accounts_info, &[&seeds[..]])?;
                }
            }
        }

        for (address, storage) in storage {
            for (key, value) in storage {
                if key < U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT) {
                    let index: usize = key.as_usize() * 32;
                    
                    let contract = self.ethereum_contract_mut(&address);
                    value.to_big_endian(&mut contract.storage[index..index+32]);
                } else {
                    self.update_storage_infinite(address, key, value, operator, system_program)?;
                }
            }
        }

        debug_print!("Applies done");

        Ok(())
    }

    /// Delete all data in the account.
    fn delete_account(&mut self, address: H160) -> ProgramResult {
        let account = self.ethereum_account_mut(&address);

        assert_eq!(account.balance, U256::zero()); // balance should be moved by executor
        account.trx_count = 0;
        account.generation = account.generation.checked_add(1)
            .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - generation overflow", address))?;


        unsafe {
            // Release borrowed account data
            ManuallyDrop::drop(
                &mut std::mem::replace(&mut account.extension, ManuallyDrop::new(None))
            );
        }

        account.info.realloc(EthereumAccount::SIZE, false)?;

        Ok(())
    }

    fn deploy_contract(
        &mut self,
        address: H160,
        code: &[u8],
        valids: &[u8],
        system_program: &program::System<'a>,
        payer: &Operator<'a>,
    ) -> ProgramResult {
        solana_program::msg!("deploy_contract: begin");
        let account = self.ethereum_accounts.get_mut(&address)
            .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address))?;

        solana_program::msg!("deploy_contract: 1");

        let old_code_size = account.extension.as_mut()
            .map_or(0, |extension| extension.code_size());

        solana_program::msg!("deploy_contract: 2");

        if old_code_size != code.len() {
            solana_program::msg!("deploy_contract: 3");
            unsafe { account.drop_extension(); }
            solana_program::msg!("deploy_contract: 4");

            let mut cur_len = account.info.data_len();
            let new_len = EthereumAccount::SIZE +
                ContractExtension::size_needed(code.len());

            let rent = Rent::get()?;
            solana_program::msg!("deploy_contract: 5");

            let balance_needed = rent.minimum_balance(new_len);

            if account.info.lamports() < balance_needed {
                solana_program::msg!("deploy_contract: 6");
                invoke(
                    &system_instruction::transfer(
                        payer.key,
                        account.info.key,
                        balance_needed - account.info.lamports(),
                    ),
                    &[
                        (*payer).clone(),
                        account.info.clone(),
                        (*system_program).clone(),
                    ],
                )?;
            }

            solana_program::msg!("deploy_contract: 7");

            if new_len < cur_len {
                solana_program::msg!("deploy_contract: 8");
                account.info.realloc(new_len, false)?;
            } else {
                solana_program::msg!("deploy_contract: 9");
                while cur_len < new_len {
                    solana_program::msg!("deploy_contract: 10");
                    cur_len += min(new_len - cur_len, MAX_PERMITTED_DATA_INCREASE);
                    account.info.realloc(cur_len, false)?;
                }
            }

            solana_program::msg!("deploy_contract: 11");

            account.reload_extension()?;

            solana_program::msg!("deploy_contract: 12");

            {
                let extension = account.extension.as_mut().unwrap();
                solana_program::msg!("deploy_contract: 13");
                extension.update_code_size(
                    code.len().try_into().expect("code.len() never exceeds u32::max"),
                );
                solana_program::msg!("deploy_contract: 14");

                unsafe { account.drop_extension(); }
                solana_program::msg!("deploy_contract: 15");
                account.reload_extension()?;
            }

            solana_program::msg!("deploy_contract: 16");

            if account.info.lamports() > balance_needed {
                solana_program::msg!("deploy_contract: 17");
                let diff = account.info.lamports().saturating_sub(balance_needed);
                **account.info.lamports.borrow_mut() = balance_needed;
                **payer.lamports.borrow_mut() += diff;
            }
        }

        solana_program::msg!("deploy_contract: 18");

        let extension = account.extension.as_mut().unwrap();

        solana_program::msg!("deploy_contract: 19");

        extension.code.copy_from_slice(code);

        solana_program::msg!("deploy_contract: 20");

        extension.valids.copy_from_slice(valids);

        solana_program::msg!("deploy_contract: end");

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
        let (solana_address, bump_seed) = self.get_storage_address(&address, &index);
        let account = self.solana_accounts.get(&solana_address)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - storage account not found", solana_address))?;

        if account.owner == self.program_id {
            let mut storage = EthereumStorage::from_account(self.program_id, account)?;
            storage.value = value;

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

            EthereumStorage::init(account, crate::account::ether_storage::Data { value })?;

            return Ok(())
        }

        Err!(ProgramError::InvalidAccountData; "Account {} - expected system or program owned", solana_address)
    }


    fn transfer_neon_tokens(&mut self, source: H160, target: H160, value: U256) -> ProgramResult {
        solana_program::msg!("Transfer {} NEONs from {} to {}", value, source, target);

        if source == target {
            return Ok(())
        }

        if !self.ethereum_accounts.contains_key(&source) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expect initialized", source);
        }
        if !self.ethereum_accounts.contains_key(&target) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expect initialized", source);
        }

        if self.balance(&source) < value {
            return Err!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, required = {}", source, value)
        }

        self.ethereum_account_mut(&source).balance -= value;
        self.ethereum_account_mut(&target).balance += value;

        Ok(())
    }
    
}
