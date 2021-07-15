use std::convert::Infallible;
use std::mem;

use evm::{
    backend::Backend, Capture, ExitError, ExitFatal, ExitReason,
    gasometer, H160, H256, Handler, Resolve, Valids, U256,
};
use evm_runtime::{Control, save_created_address, save_return_value};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use crate::executor_state::{ExecutorState, StackState};
use crate::storage_account::StorageAccount;
use crate::utils::{keccak256_h256, keccak256_h256_v};

/// "All but one 64th" operation.
/// See also EIP-150.
const fn l64(gas: u64) -> u64 {
    gas - gas / 64
}

#[derive(Debug)]
struct CallInterrupt {
    context: evm::Context,
    transfer: Option<evm::Transfer>,
    code_address: H160,
    input: Vec<u8>,
    gas_limit: u64,
}

#[derive(Debug)]
struct CreateInterrupt {
    context: evm::Context,
    transfer: Option<evm::Transfer>,
    address: H160,
    init_code: Vec<u8>,
    gas_limit: u64,
}

enum RuntimeApply{
    Continue,
    Call(CallInterrupt),
    Create(CreateInterrupt),
    Exit(ExitReason),
}

struct Executor<'config, B: Backend> {
    state: ExecutorState<'config, B>,
    config: &'config evm::Config,
}

impl<'config, B: Backend> Handler for Executor<'config, B> {
    type CreateInterrupt = crate::executor::CreateInterrupt;
    type CreateFeedback = Infallible;
    type CallInterrupt = crate::executor::CallInterrupt;
    type CallFeedback = Infallible;

    fn keccak256_h256(&self, data: &[u8]) -> H256 {
        keccak256_h256(data)
    }

    fn balance(&self, address: H160) -> U256 {
        self.state.basic(address).balance
    }

    fn code_size(&self, address: H160) -> U256 {
        U256::from(self.state.code_size(address))
    }

    fn code_hash(&self, address: H160) -> H256 {
        if self.exists(address) {
            self.state.code_hash(address)
        } else {
            H256::default()
        }
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.state.code(address)
    }

    fn valids(&self, address: H160) -> Vec<u8> {
        self.state.valids(address)
    }

    fn storage(&self, address: H160, index: U256) -> U256 {
        self.state.storage(address, index)
    }

    fn original_storage(&self, address: H160, index: U256) -> U256 {
        self.state.original_storage(address, index).unwrap_or_default()
    }

    fn gas_left(&self) -> U256 {
        U256::from(self.state.metadata().gasometer().gas()) // U256::one()
    }

    fn gas_price(&self) -> U256 {
        self.state.gas_price()
    }

    fn origin(&self) -> H160 {
        self.state.origin()
    }

    fn block_hash(&self, number: U256) -> H256 {
        self.state.block_hash(number)
    }

    fn block_number(&self) -> U256 {
        self.state.block_number()
    }

    fn block_coinbase(&self) -> H160 {
        self.state.block_coinbase()
    }

    fn block_timestamp(&self) -> U256 {
        self.state.block_timestamp()
    }

    fn block_difficulty(&self) -> U256 {
        self.state.block_difficulty()
    }

    fn block_gas_limit(&self) -> U256 {
        self.state.block_gas_limit()
    }

    fn chain_id(&self) -> U256 {
        self.state.chain_id()
    }

    fn exists(&self, address: H160) -> bool {
        if self.config.empty_considered_exists {
            self.state.exists(address)
        } else {
            self.state.exists(address) && !self.state.is_empty(address)
        }
    }

    fn deleted(&self, address: H160) -> bool {
        self.state.deleted(address)
    }

    fn set_storage(&mut self, address: H160, index: U256, value: U256) -> Result<(), ExitError> {
        self.state.set_storage(address, index, value);
        Ok(())
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
        self.state.log(address, topics, data);
        Ok(())
    }

    fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
        let balance = self.balance(address);
        let transfer = evm::Transfer {
            source: address,
            target,
            value: balance,
        };

        self.state.transfer(&transfer)?;
        self.state.reset_balance(address);
        self.state.set_deleted(address);

        Ok(())
    }

    fn create(
        &mut self,
        caller: H160,
        scheme: evm::CreateScheme,
        value: U256,
        init_code: Vec<u8>,
        target_gas: Option<u64>,
    ) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
        debug_print!("create target_gas={:?}", target_gas);
        if let Some(depth) = self.state.metadata().depth() {
            if depth + 1 > self.config.call_stack_limit {
                return Capture::Exit((ExitError::CallTooDeep.into(), None, Vec::new()));
            }
        }

        if self.balance(caller) < value {
            return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
        }

        let after_gas = if self.config.call_l64_after_gas {
            if self.config.estimate {
                let initial_after_gas = self.state.metadata().gasometer().gas();
                let diff = initial_after_gas - l64(initial_after_gas);
                if let Err(e) = self.state.metadata_mut().gasometer_mut().record_cost(diff) {
                    return Capture::Exit((e.into(), None, Vec::new()));
                }
                self.state.metadata().gasometer().gas()
            } else {
                l64(self.state.metadata().gasometer().gas())
            }
        } else {
            self.state.metadata().gasometer().gas()
        };

        let target_gas = target_gas.unwrap_or(after_gas);

        let gas_limit = core::cmp::min(target_gas, after_gas);
        if let Err(e) = self.state.metadata_mut().gasometer_mut().record_cost(gas_limit) {
            return Capture::Exit((e.into(), None, Vec::new()));
        }

        // Get the create address from given scheme.
        let address =
            match scheme {
                evm::CreateScheme::Create2 { caller, code_hash, salt } => {
                    keccak256_h256_v(&[&[0xff], &caller[..], &salt[..], &code_hash[..]]).into()
                },
                evm::CreateScheme::Legacy { caller } => {
                    let nonce = self.state.basic(caller).nonce;
                    let mut stream = rlp::RlpStream::new_list(2);
                    stream.append(&caller);
                    stream.append(&nonce);
                    keccak256_h256(&stream.out()).into()
                },
                evm::CreateScheme::Fixed(naddress) => {
                    naddress
                },
            };

        self.state.create(&scheme, &address);
        // TODO: may be increment caller's nonce after runtime creation or success execution?
        self.state.inc_nonce(caller);

        let existing_code = self.state.code(address);
        if !existing_code.is_empty() {
            // let _ = self.merge_fail(substate);
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        if self.state.basic(address).nonce  > U256::zero() {
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        let context = evm::Context {
            address,
            caller,
            apparent_value: value,
        };

        let transfer = Some(evm::Transfer { source: caller, target: address, value });

        Capture::Trap(CreateInterrupt{context, transfer, address, init_code, gas_limit})
    }

    fn call(
        &mut self,
        code_address: H160,
        transfer: Option<evm::Transfer>,
        input: Vec<u8>,
        target_gas: Option<u64>,
        is_static: bool,
        context: evm::Context,
    ) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
        debug_print!("call target_gas={:?}", target_gas);
        if let Some(depth) = self.state.metadata().depth() {
            if depth + 1 > self.config.call_stack_limit {
                return Capture::Exit((ExitError::CallTooDeep.into(), Vec::new()));
            }
        }

        // These parameters should be true for call from another contract
        let take_l64 = true;
        let take_stipend = true;

        let after_gas = if take_l64 && self.config.call_l64_after_gas {
            if self.config.estimate {
                let initial_after_gas = self.state.metadata().gasometer().gas();
                let diff = initial_after_gas - l64(initial_after_gas);
                if let Err(e) = self.state.metadata_mut().gasometer_mut().record_cost(diff) {
                    return Capture::Exit((e.into(), Vec::new()));
                }
                self.state.metadata().gasometer().gas()
            } else {
                l64(self.state.metadata().gasometer().gas())
            }
        } else {
            self.state.metadata().gasometer().gas()
        };

        let target_gas = target_gas.unwrap_or(after_gas);
        let mut gas_limit = core::cmp::min(target_gas, after_gas);

        if let Err(e) = self.state.metadata_mut().gasometer_mut().record_cost(gas_limit) {
            return Capture::Exit((e.into(), Vec::new()));
        }

        if let Some(transfer) = transfer.as_ref() {
            if take_stipend && transfer.value != U256::zero() {
                gas_limit = gas_limit.saturating_add(self.config.call_stipend);
            }
        }

        let hook_res = self.state.call_inner(code_address, transfer, input.clone(), Some(gas_limit), is_static, take_l64, take_stipend);
        if hook_res.is_some() {
            match hook_res.as_ref().unwrap() {
                Capture::Exit((reason, return_data)) => {
                    return Capture::Exit((*reason, return_data.clone()))
                },
                Capture::Trap(_interrupt) => {
                    unreachable!("not implemented");
                },
            }
        }

        Capture::Trap(CallInterrupt{context, transfer, code_address, input, gas_limit})
    }

    fn pre_validate(
        &mut self,
        context: &evm::Context,
        opcode: evm::Opcode,
        stack: &evm::Stack,
    ) -> Result<(), ExitError> {
        if let Some(cost) = gasometer::static_opcode_cost(opcode) {
            self.state
                .metadata_mut()
                .gasometer_mut()
                .record_cost(cost)?;
        } else {
            let is_static = self.state.metadata().is_static();
            let (gas_cost, memory_cost) = gasometer::dynamic_opcode_cost(
                context.address,
                opcode,
                stack,
                is_static,
                self.config,
                self,
            )?;

            self.state.metadata_mut().gasometer_mut().record_dynamic_cost(gas_cost, memory_cost)?;
        }

        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum CreateReason {
    Call,
    Create(H160),
}

type RuntimeInfo<'config> = (evm::Runtime<'config>, CreateReason);

pub struct Machine<'config, B: Backend> {
    executor: Executor<'config, B>,
    runtime: Vec<RuntimeInfo<'config>>
}

impl<'config, B: Backend> Machine<'config, B> {

    pub fn new(state: ExecutorState<'config, B>) -> Self {
        let executor = Executor { state, config: evm::Config::default() };
        Self{ executor, runtime: Vec::new() }
    }

    pub fn save_into(&self, storage: &mut StorageAccount) {
        storage.serialize(&self.runtime, self.executor.state.substate()).unwrap();
    }

    pub fn restore(storage: &StorageAccount, backend: B) -> Self {
        let (runtime, substate) = storage.deserialize().unwrap();

        let state = ExecutorState::new(substate, backend);

        let executor = Executor { state, config: evm::Config::default() };
        Self{ executor, runtime }
    }

    pub fn call_begin(&mut self,
        caller: H160,
        code_address: H160,
        input: Vec<u8>,
        transfer_value: U256,
        gas_limit: u64,
    ) -> ProgramResult {
        debug_print!("call_begin gas_limit={}", gas_limit);

        let transaction_cost = gasometer::call_transaction_cost(&input);
        self.executor.state.metadata_mut().gasometer_mut().record_transaction(transaction_cost)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        let after_gas = self.executor.state.metadata().gasometer().gas();
        let gas_limit = core::cmp::min(gas_limit, after_gas);

        self.executor.state.metadata_mut().gasometer_mut().record_cost(gas_limit)
            .map_err(|_| ProgramError::InvalidInstructionData)?;


        self.executor.state.inc_nonce(caller);

        self.executor.state.enter(gas_limit, false);
        self.executor.state.touch(code_address);

        let transfer = evm::Transfer { source: caller, target: code_address, value: transfer_value };
        self.executor.state.transfer(&transfer).map_err(|_| ProgramError::InsufficientFunds)?;

        let code = self.executor.code(code_address);
        let valids = self.executor.valids(code_address);
        let context = evm::Context{address: code_address, caller, apparent_value: U256::zero()};

        let runtime = evm::Runtime::new(code, valids, input, context, self.executor.config);

        self.runtime.push((runtime, CreateReason::Call));

        Ok(())
    }

    pub fn create_begin(&mut self,
                        caller: H160,
                        code: Vec<u8>,
                        transfer_value: U256,
                        gas_limit: u64,
    ) -> ProgramResult {
        debug_print!("create_begin gas_limit={}", gas_limit);
        let transaction_cost = gasometer::create_transaction_cost(&code);
        self.executor.state.metadata_mut().gasometer_mut()
            .record_transaction(transaction_cost)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        let after_gas = self.executor.state.metadata().gasometer().gas();
        let gas_limit = core::cmp::min(gas_limit, after_gas);

        self.executor.state.metadata_mut().gasometer_mut().record_cost(gas_limit)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        let scheme = evm::CreateScheme::Legacy { caller };
        self.executor.state.enter(gas_limit, false);

        match self.executor.create(caller, scheme, transfer_value, code, None) {
            Capture::Exit(_) => {
                debug_print!("create_begin() error ");
                return Err(ProgramError::InvalidInstructionData);
            },
            Capture::Trap(info) => {
                self.executor.state.touch(info.address);
                self.executor.state.reset_storage(info.address);
                if self.executor.config.create_increase_nonce {
                    self.executor.state.inc_nonce(info.address);
                }

                if let Some(transfer) = info.transfer {
                    self.executor.state.transfer(&transfer).map_err(|_| ProgramError::InsufficientFunds)?;
                }

                let valids = Valids::compute(&info.init_code);
                let instance = evm::Runtime::new(
                    info.init_code,
                    valids,
                    Vec::new(),
                    info.context,
                    self.executor.config
                );
                self.runtime.push((instance, CreateReason::Create(info.address)));
            },
        }

        Ok(())
    }

    fn run(&mut self, max_steps: u64) -> (u64, RuntimeApply) {
        let runtime = match self.runtime.last_mut() {
            Some((runtime, _)) => runtime,
            None => return (0, RuntimeApply::Exit(ExitFatal::NotSupported.into()))
        };

        let (steps_executed, capture) = runtime.run(max_steps, &mut self.executor);
        match capture {
            Capture::Exit(ExitReason::StepLimitReached) => (steps_executed, RuntimeApply::Continue),
            Capture::Exit(reason) => (steps_executed, RuntimeApply::Exit(reason)),
            Capture::Trap(interrupt) => {
                match interrupt {
                    Resolve::Call(interrupt, resolve) => {
                        mem::forget(resolve);
                        (steps_executed, RuntimeApply::Call(interrupt))
                    },
                    Resolve::Create(interrupt, resolve) => {
                        mem::forget(resolve);
                        (steps_executed, RuntimeApply::Create(interrupt))
                    },
                }
            }
        }
    }

    fn apply_call(&mut self, interrupt: CallInterrupt) -> Result<(), (Vec<u8>, ExitReason)> {
        debug_print!("apply_call {:?}", interrupt);
        let code = self.executor.code(interrupt.code_address);
        let valids = self.executor.valids(interrupt.code_address);

        self.executor.state.enter(interrupt.gas_limit, false);
        self.executor.state.touch(interrupt.code_address);

        if let Some(transfer) = interrupt.transfer {
            self.executor.state.transfer(&transfer).map_err(|_| (Vec::new(), ExitError::OutOfFund.into()))?;
        }

        let instance = evm::Runtime::new(
            code,
            valids,
            interrupt.input,
            interrupt.context,
            self.executor.config
        );
        self.runtime.push((instance, CreateReason::Call));

        Ok(())
    }

    fn apply_create(&mut self, interrupt: CreateInterrupt) -> Result<(), (Vec<u8>, ExitReason)> {
        debug_print!("apply_create {:?}", interrupt);
        self.executor.state.enter(interrupt.gas_limit, false);
        self.executor.state.touch(interrupt.address);
        self.executor.state.reset_storage(interrupt.address);
        if self.executor.config.create_increase_nonce {
            self.executor.state.inc_nonce(interrupt.address);
        }

        if let Some(transfer) = interrupt.transfer {
            self.executor.state.transfer(&transfer).map_err(|_| (Vec::new(), ExitError::OutOfFund.into()))?;
        }

        let valids = Valids::compute(&interrupt.init_code);
        let instance = evm::Runtime::new(
            interrupt.init_code,
            valids,
            Vec::new(),
            interrupt.context,
            self.executor.config
        );
        self.runtime.push((instance, CreateReason::Create(interrupt.address)));

        Ok(())
    }

    fn apply_exit_call(&mut self, exited_runtime: &evm::Runtime, reason: ExitReason) -> Result<(), (Vec<u8>, ExitReason)> {
        if reason.is_succeed() {
            self.executor.state.exit_commit().map_err(|e| (Vec::new(), ExitReason::from(e)))?;
        }
        
        let return_value = exited_runtime.machine().return_value();
        if self.runtime.is_empty() {
            return Err((return_value, reason));
        }

        let (runtime, _) = self.runtime.last_mut().unwrap();

        match save_return_value(runtime, reason, return_value, &self.executor) {
            Control::Continue => Ok(()),
            Control::Exit(reason) => Err((Vec::new(), reason)),
            _ => unreachable!()
        }
    }

    fn apply_exit_create(&mut self, exited_runtime: &evm::Runtime, mut reason: ExitReason, address: H160) -> Result<(), (Vec<u8>, ExitReason)> {
        let return_value = exited_runtime.machine().return_value();

        if reason.is_succeed() {
            match self.executor.config.create_contract_limit {
                Some(limit) if return_value.len() > limit => {
                    self.executor.state.exit_discard().map_err(|e| (Vec::new(), ExitReason::from(e)))?;
                    reason = ExitError::CreateContractLimit.into();
                },
                _ => {
                    self.executor.state.exit_commit().map_err(|e| (Vec::new(), ExitReason::from(e)))?;
                    self.executor.state.set_code(address, return_value);
                }
            };
        }

        let runtime = match self.runtime.last_mut() {
            Some((runtime, _)) => runtime,
            None => return Err((Vec::new(), reason))
        };
        match save_created_address(runtime, reason, Some(address), &self.executor) {
            Control::Continue => Ok(()),
            Control::Exit(reason) => Err((Vec::new(), reason)),
            _ => unreachable!()
        }
    }

    fn apply_exit(&mut self, reason: ExitReason) -> Result<(), (Vec<u8>, ExitReason)> {
        match reason {
            ExitReason::Succeed(_) => Ok(()),
            ExitReason::Revert(_) => self.executor.state.exit_revert(),
            ExitReason::Error(_) | ExitReason::Fatal(_) => self.executor.state.exit_discard(),
            ExitReason::StepLimitReached => unreachable!()
        }.map_err(|e| (Vec::new(), ExitReason::from(e)))?;

        let (exited_runtime, create_reason) = match self.runtime.pop() {
            Some((runtime, reason)) => (runtime, reason),
            None => return Err((Vec::new(), ExitFatal::NotSupported.into()))
        };

        match create_reason {
            CreateReason::Call => self.apply_exit_call(&exited_runtime, reason),
            CreateReason::Create(address) => self.apply_exit_create(&exited_runtime, reason, address)
        }
    }

    pub fn execute(&mut self) -> (Vec<u8>, ExitReason) {
        loop {
            if let Err(result) = self.execute_n_steps(u64::max_value()) {
                return result;
            }
        }
    }

    pub fn execute_n_steps(&mut self, n: u64) -> Result<(), (Vec<u8>, ExitReason)> {
        let mut steps = 0_u64;

        while steps < n {
            let (steps_executed, apply) = self.run(n - steps);
            steps += steps_executed;

            match apply {
                RuntimeApply::Continue => {},
                RuntimeApply::Call(info) => self.apply_call(info)?,
                RuntimeApply::Create(info) => self.apply_create(info)?,
                RuntimeApply::Exit(reason) => self.apply_exit(reason)?,
            }
        }

        Ok(())
    }

    pub fn into_state(self) -> ExecutorState<'config, B> {
        self.executor.state
    }
}
