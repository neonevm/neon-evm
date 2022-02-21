use std::collections::BTreeMap;
use std::convert::TryInto;
use evm::{H160, U256};
use evm::backend::Apply;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use crate::account::{ACCOUNT_SEED_VERSION, ERC20Allowance, EthereumAccount, Operator, program};
use crate::account_storage::{Account, AccountStorage, ProgramAccountStorage};
use crate::executor_state::{ApplyState, ERC20Approve, SplApprove, SplTransfer, Withdraw};
use crate::precompile_contracts::is_precompile_address;
use solana_program::program::invoke_signed;
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use solana_program::sysvar::rent;

impl<'a> ProgramAccountStorage<'a> {
    pub fn transfer_gas_payment(
        &mut self,
        origin: H160,
        mut operator: EthereumAccount<'a>,
        used_gas: U256,
        gas_price: U256,
    ) -> Result<(), ProgramError> {
        // Can overflow in malicious transaction
        let value = used_gas.saturating_mul(gas_price);

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
            self.apply_withdrawals(withdrawals, operator, system_program)?;
        }

        if !applies.is_empty() {
            self.apply_contract_results(applies, operator)?;
        }

        debug_print!("Applies done");

        for log in logs {
            neon_program.on_event(log)?;
        }

        Ok(())
    }

    /// Delete all data in the account. Move lamports to the operator
    fn delete_account(&mut self, address: H160, operator: &Operator<'a>) -> Result<(), ProgramError> {
        if let Some(account) = self.ethereum_accounts.remove(&address) {
            let (account, contract) = account.deconstruct();

            assert_eq!(account.balance, U256::zero()); // balance should be moved by executor
            assert!(contract.is_some()); // can only be deleted by calling suicide() in contract code

            unsafe {
                account.suicide(operator)?;
                contract.unwrap().suicide(operator)?;
            }
        } else {
            // Never happens. Create a bug if you see this.
            panic!("Attempt to delete not initialized account {}", address);
        }

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
            contract.extension.storage.clear();
        } else {
            return Err!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address);
        }

        Ok(())
    }

    fn update_account(
        &mut self,
        address: H160,
        trx_count: U256,
        code_and_valids: Option<(Vec<u8>, Vec<u8>)>,
        storage: BTreeMap<U256, U256>,
        reset_storage: bool
    ) -> Result<(), ProgramError> {
        if self.nonce(&address) != trx_count {
            let account = self.ethereum_account_mut(&address)
                .ok_or_else(|| E!(ProgramError::UninitializedAccount; "Account {} - is not initialized", address))?;

            assert!(trx_count > U256::from(account.trx_count));
            account.trx_count = (trx_count % U256::from(u64::MAX)).as_u64();
        }

        if let Some((code, valids)) = code_and_valids {
            self.deploy_contract(address, &code, &valids)?;
        }

        if reset_storage | !storage.is_empty() {
            let contract = self.ethereum_contract_mut(&address)
                .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - is not contract", address))?;

            if reset_storage {
                contract.extension.storage.clear();
            }

            for (key, value) in storage {
                contract.extension.storage.insert(key, value)?;
            }
        }

        Ok(())
    }

    fn apply_contract_results(
        &mut self,
        values: Vec<Apply<BTreeMap<U256, U256>>>,
        operator: &Operator<'a>,
    ) -> Result<(), ProgramError> {
        debug_print!("apply_contract_results");

        for apply in values {
            match apply {
                Apply::Modify {address, nonce, code_and_valids, storage, reset_storage} => {
                    if is_precompile_address(&address) {
                        continue;
                    }

                    self.update_account(address, nonce, code_and_valids, storage, reset_storage)?;
                },
                Apply::Delete { address } => {
                    self.delete_account(address, operator)?;
                }
            }
        }

        Ok(())
    }

    fn transfer_neon_tokens(&mut self, source: H160, target: H160, value: U256) -> Result<(), ProgramError> {
        solana_program::msg!("Transfer {} NEONs from {} to {}", value, source, target);

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

    fn apply_withdrawals(&mut self,
                         withdrawals: Vec<Withdraw>,
                         operator: &Operator<'a>,
                         system_program: &program::System<'a>) -> Result<(), ProgramError> {
        debug_print!("apply_withdrawals {:?}", withdrawals);

        debug_print!("operator: {:?}", operator.key);

        let (authority, bump_seed) = Pubkey::find_program_address(&[b"Deposit"], self.program_id);
        debug_print!("deposit_authority {:?}", authority);

        let pool_address = get_associated_token_address(
            &authority,
            &crate::config::token_mint::id()
        );

        debug_print!("deposit_pool_address {:?}", pool_address);

        let signers_seeds: &[&[&[u8]]] = &[&[b"Deposit", &[bump_seed]]];

        for withdraw in withdrawals {

            debug_print!("destination {:?}", withdraw.dest);
            debug_print!("dest_neon {:?}", withdraw.dest_neon);

            let dest_neon = self.solana_accounts[&withdraw.dest_neon];

            if dest_neon.data_is_empty() {
                let create_acc_insrt = create_associated_token_account(operator.key,
                                                                       &withdraw.dest,
                                                                       &crate::config::token_mint::id());

                let account_infos: &[AccountInfo] = &[
                    (**operator).clone(),
                    dest_neon.clone(),
                    self.solana_accounts[&withdraw.dest].clone(),
                    self.solana_accounts[&crate::config::token_mint::id()].clone(),
                    (*system_program).clone(),
                    self.solana_accounts[&spl_token::id()].clone(),
                    self.solana_accounts[&rent::id()].clone(),
                    self.solana_accounts[&spl_associated_token_account::id()].clone(),
                ];

                invoke_signed(&create_acc_insrt, account_infos, signers_seeds)?;
            };

            let transfer_instr = spl_token::instruction::transfer(
                &spl_token::id(),
                &pool_address,
                &dest_neon.key,
                &authority,
                &[],
                withdraw.amount.as_u64()
            )?;

            let account_infos: &[AccountInfo] = &[
                //(**operator).clone(),
                self.solana_accounts[&pool_address].clone(),
                dest_neon.clone(),
                self.solana_accounts[&authority].clone(),
                self.solana_accounts[&spl_token::id()].clone()
            ];

            invoke_signed(&transfer_instr, account_infos, signers_seeds)?;
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