use std::collections::BTreeMap;
use std::convert::TryInto;
use evm::{H160, U256};
use evm::backend::Apply;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::account::{ACCOUNT_SEED_VERSION, ERC20Allowance, EthereumAccount, EthereumStorage, Operator, program};
use crate::account_storage::{Account, AccountStorage, ProgramAccountStorage};
use crate::executor_state::{ApplyState, ERC20Approve, SplApprove, SplTransfer, Withdraw};
use crate::precompile_contracts::is_precompile_address;
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use solana_program::program::invoke_signed;
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use solana_program::sysvar::rent;


impl<'a> ProgramAccountStorage<'a> {
    pub fn transfer_gas_payment(
        &mut self,
        origin: H160,
        mut operator: EthereumAccount<'a>,
        value: U256,
    ) -> Result<(), ProgramError> {
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
            let origin_account = self.ethereum_account_mut(&origin)
                .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - expect initialized", origin))?;

            let origin_balance = origin_account.balance.checked_sub(value)
                .ok_or_else(|| E!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, balance = {}", origin, origin_account.balance))?;

            let operator_balance = operator.balance.checked_add(value)
                .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - balance overflow", operator.address))?;

            origin_account.balance = origin_balance;
            operator.balance = operator_balance;
        }

        Ok(())
    }

    pub fn apply_state_change(
        &mut self,
        neon_program: &program::Neon<'a>,
        system_program: &program::System<'a>,
        operator: &Operator<'a>,
        state: ApplyState,
    ) -> Result<(), ProgramError> {
        let (
            applies,
            logs,
            transfers,
            spl_transfers,
            spl_approves,
            withdrawals,
            erc20_approves,
        ) = state;

        debug_print!("Applies begin");

        if !transfers.is_empty() {
            self.apply_transfers(transfers)?;
        }

        if !spl_approves.is_empty() {
            self.apply_spl_approves(spl_approves)?;
        }

        if !spl_transfers.is_empty() {
            self.apply_spl_transfers(spl_transfers)?;
        }

        if !erc20_approves.is_empty() {
            self.apply_erc20_approves( operator, system_program, erc20_approves)?;
        }

        if !withdrawals.is_empty() {
            self.apply_withdrawals(withdrawals, operator)?;
        }

        if !applies.is_empty() {
            self.apply_contract_results(applies, operator, system_program)?;
        }

        debug_print!("Applies done");

        for log in logs {
            neon_program.on_event(log)?;
        }

        Ok(())
    }

    /// Delete all data in the account.
    fn delete_account(&mut self, address: H160) -> Result<(), ProgramError> {
        let account = self.ethereum_account_mut(&address)
            .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - expected initialized account", address))?;

        assert_eq!(account.balance, U256::zero()); // balance should be moved by executor
        account.trx_count = 0;


        let contract = self.ethereum_contract_mut(&address)
            .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - expected contract account", address))?;

        contract.code_size = 0;
        contract.generation = contract.generation.checked_add(1)
            .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - generation overflow", address))?;

        contract.extension.code.fill(0);
        contract.extension.valids.fill(0);
        contract.extension.storage.fill(0);

        Ok(())
    }

    fn deploy_contract(&mut self, address: H160, code: &[u8], valids: &[u8]) -> Result<(), ProgramError> {
        if let Some(account) = self.ethereum_accounts.get_mut(&address) {

            let contract = match account {
                Account::User(_) => return Err!(ProgramError::InvalidArgument; "Account {} - is not contract account", address),
                Account::Contract(_, contract) => contract
            };

            contract.code_size = code.len().try_into().expect("code.len() never exceeds u32::max");

            contract.reload_extension()?;
            contract.extension.code.copy_from_slice(code);
            contract.extension.valids.copy_from_slice(valids);
        } else {
            return Err!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address);
        }

        Ok(())
    }

    pub fn update_storage_infinite(
        &mut self,
        address: H160,
        index: U256,
        value: U256,
        operator: &Operator<'a>,
        system_program: &program::System<'a>,
    ) -> Result<(), ProgramError> {
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

        return Err!(ProgramError::InvalidAccountData; "Account {} - expected system or program owned", solana_address);
    }


    fn update_account_storage(
        &mut self,
        address: H160,
        mut storage: BTreeMap<U256, U256>,
        reset_storage: bool,
        operator: &Operator<'a>,
        system_program: &program::System<'a>,
    ) -> Result<(), ProgramError> {
        if reset_storage | !storage.is_empty() {

            if reset_storage {
                let contract = self.ethereum_contract_mut(&address)
                    .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - is not contract", address))?;

                contract.generation = contract.generation.checked_add(1)
                    .ok_or_else(|| E!(ProgramError::InvalidInstructionData; "Account {} - generation overflow", address))?;

                contract.extension.storage.fill(0);
            }
            
            let infinite_storage = storage.split_off(&U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT));

            if !storage.is_empty() {
                let contract = self.ethereum_contract_mut(&address)
                    .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - is not contract", address))?;

                for (index, value) in storage {
                    let index: usize = index.as_usize() * 32;
                    value.to_big_endian(&mut contract.extension.storage[index..index+32]);
                }
            }


            for (index, value) in infinite_storage {
                self.update_storage_infinite(address, index, value, operator, system_program)?;
            }

        }

        Ok(())
    }

    fn update_account_trx_count(
        &mut self,
        address: H160,
        trx_count: U256,
    ) -> Result<(), ProgramError> {
        if self.nonce(&address) != trx_count {
            let account = self.ethereum_account_mut(&address)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address))?;

            assert!(trx_count > U256::from(account.trx_count));
            if trx_count > U256::from(u64::MAX) {
                return Err!(ProgramError::InvalidInstructionData; "Account {} - nonce overflow", address);
            }

            account.trx_count = trx_count.as_u64();
        }

        Ok(())
    }

    fn apply_contract_results(
        &mut self,
        values: Vec<Apply<BTreeMap<U256, U256>>>,
        operator: &Operator<'a>,
        system_program: &program::System<'a>,
    ) -> Result<(), ProgramError> {
        debug_print!("apply_contract_results");

        for apply in values {
            match apply {
                Apply::Modify {address, nonce, code_and_valids, storage, reset_storage} => {
                    if is_precompile_address(&address) {
                        continue;
                    }

                    self.update_account_trx_count(address, nonce)?;

                    if let Some((code, valids)) = code_and_valids {
                        self.deploy_contract(address, &code, &valids)?;
                    }

                    self.update_account_storage(address, storage, reset_storage, operator, system_program)?;
                },
                Apply::Delete { address } => {
                    self.delete_account(address)?;
                }
            }
        }

        Ok(())
    }

    fn transfer_neon_tokens(&mut self, source: H160, target: H160, value: U256) -> Result<(), ProgramError> {
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

        let source_balance = self.balance(&source).checked_sub(value)
            .ok_or_else(|| E!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, balance = {}", source, self.balance(&source)))?;

        let target_balance = self.balance(&target).checked_add(value)
            .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - balance overflow", target))?;

        self.ethereum_account_mut(&source)
            .unwrap() // checked before
            .balance = source_balance;

        self.ethereum_account_mut(&target)
            .unwrap() // checked before
            .balance = target_balance;

        Ok(())
    }

    fn apply_transfers(&mut self, transfers: Vec<evm::Transfer>) -> Result<(), ProgramError> {
        debug_print!("apply_transfers {:?}", transfers);

        for transfer in transfers {
            self.transfer_neon_tokens(transfer.source, transfer.target, transfer.value)?;
        }

        Ok(())
    }

    fn apply_spl_transfers(&mut self, transfers: Vec<SplTransfer>) -> Result<(), ProgramError> {
        debug_print!("apply_spl_transfers {:?}", transfers);

        let token_program = self.token_program.as_ref()
            .ok_or_else(|| E!(ProgramError::MissingRequiredSignature; "Token program not found"))?;


        for transfer in transfers {
            let authority = self.ethereum_account(&transfer.source)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Solidity account {} must be initialized", transfer.source))?;

            let source = self.solana_accounts[&transfer.source_token];
            let target = self.solana_accounts[&transfer.target_token];

            token_program.transfer(authority, source, target, transfer.value)?;
        }

        Ok(())
    }

    fn apply_spl_approves(&mut self, approves: Vec<SplApprove>) -> Result<(), ProgramError> {
        debug_print!("apply_spl_approves {:?}", approves);

        let token_program = self.token_program.as_ref()
            .ok_or_else(|| E!(ProgramError::MissingRequiredSignature; "Token program not found"))?;


        for approve in approves {
            let authority = self.ethereum_account(&approve.owner)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Solidity account {} must be initialized", approve.owner))?;

            let (source_key, _) = self.get_erc20_token_address(&approve.owner, &approve.contract, &approve.mint);
            let source = self.solana_accounts[&source_key];
            let delegate = self.solana_accounts[&approve.spender];

            token_program.approve(authority, source, delegate, approve.value)?;
        }

        Ok(())
    }

    fn apply_withdrawals(
        &mut self,
        withdrawals: Vec<Withdraw>,
        operator: &Operator<'a>,
    ) -> Result<(), ProgramError> {
        debug_print!("apply_withdrawals {:?}", withdrawals);

        let (authority, bump_seed) = Pubkey::find_program_address(&[b"Deposit"], self.program_id);

        let pool_address = get_associated_token_address(
            &authority,
            self.token_mint()
        );

        let signers_seeds: &[&[&[u8]]] = &[&[b"Deposit", &[bump_seed]]];

        for withdraw in withdrawals {
            let dest_neon = self.solana_accounts[&withdraw.dest_neon];

            if dest_neon.data_is_empty() {
                let create_acc_insrt = create_associated_token_account(operator.key,
                                                                       &withdraw.dest,
                                                                       self.token_mint());

                let account_infos: &[AccountInfo] = &[
                    (**operator).clone(),
                    dest_neon.clone(),
                    self.solana_accounts[&withdraw.dest].clone(),
                    self.solana_accounts[self.token_mint()].clone(),
                    self.solana_accounts[&spl_token::id()].clone(),
                    self.solana_accounts[&rent::id()].clone(),
                    self.solana_accounts[&spl_associated_token_account::id()].clone(),
                ];

                invoke_signed(&create_acc_insrt, account_infos, signers_seeds)?;
            };

            let transfer_instr = spl_token::instruction::transfer(
                &spl_token::id(),
                &pool_address,
                dest_neon.key,
                &authority,
                &[],
                withdraw.spl_amount
            )?;

            let account_infos: &[AccountInfo] = &[
                self.solana_accounts[&pool_address].clone(),
                dest_neon.clone(),
                self.solana_accounts[&authority].clone(),
                self.solana_accounts[&spl_token::id()].clone()
            ];

            invoke_signed(&transfer_instr, account_infos, signers_seeds)?;

            let source_balance = self.balance(&withdraw.source).checked_sub(withdraw.neon_amount)
                .ok_or_else(|| E!(ProgramError::InsufficientFunds; "Account {} - insufficient funds, balance = {}", withdraw.source, self.balance(&withdraw.source)))?;

            self.ethereum_account_mut(&withdraw.source)
                .unwrap() // checked before
                .balance = source_balance;
        }

        Ok(())
    }

    fn apply_erc20_approves(
        &mut self,
        operator: &Operator<'a>,
        system_program: &program::System<'a>,
        approves: Vec<ERC20Approve>,
    ) -> Result<(), ProgramError> {
        debug_print!("apply_erc20_approves {:?}", approves);

        for approve in approves {
            let (account_address, bump_seed) = self.get_erc20_allowance_address(
                &approve.owner,
                &approve.spender,
                &approve.contract,
                &approve.mint,
            );

            let account = self.solana_accounts[&account_address];
            let mut allowance_data = if account.data_is_empty() {
                let seeds: &[&[u8]] = &[
                    &[ACCOUNT_SEED_VERSION],
                    b"ERC20Allowance",
                    &approve.mint.to_bytes(),
                    approve.contract.as_bytes(),
                    approve.owner.as_bytes(),
                    approve.spender.as_bytes(),
                    &[bump_seed]
                ];

                system_program.create_pda_account(
                    self.program_id,
                    operator,
                    account,
                    seeds,
                    ERC20Allowance::SIZE,
                )?;

                #[allow(clippy::default_trait_access)] // real type is too long
                ERC20Allowance::init(account, Default::default())
            } else {
                ERC20Allowance::from_account(self.program_id, account)
            }?;

            allowance_data.owner = approve.owner;
            allowance_data.spender = approve.spender;
            allowance_data.contract = approve.contract;
            allowance_data.mint = approve.mint;
            allowance_data.value = approve.value;
        }

        Ok(())
    }
}
