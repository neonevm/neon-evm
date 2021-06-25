use std::borrow::BorrowMut;
use std::convert::Infallible;
use std::mem;

use evm::{
    backend::Backend, Capture, ExitError, ExitFatal, ExitReason,
    gasometer, gasometer::Gasometer, H160, H256, Handler, Resolve, U256,
};
use evm_runtime::{Control, save_created_address, save_return_value};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use crate::executor_state::{ExecutorState, StackState};
use crate::storage_account::StorageAccount;
use crate::utils::{keccak256_h256, keccak256_h256_v};

const fn l64(gas: u64) -> u64 {
    gas - gas / 64
}

#[allow(clippy::cast_sign_loss)]
fn gas_used(gm: &Gasometer) -> u64 {
    let tug = gm.total_used_gas();
    let rg = gm.refunded_gas() as u64;
    tug - core::cmp::min(tug / 2, rg)
}

//fn total_gas_used(gm: &Gasometer) -> u64 {
//    gm.total_used_gas()
//}

//fn refunded_gas(gm: &Gasometer) -> i64 {
//    gm.refunded_gas()
//}

struct CallInterrupt {
    code_address : H160,
    input : Vec<u8>,
    context: evm::Context,
}

struct CreateInterrupt {
    init_code: Vec<u8>,
    context: evm::Context,
    address: H160
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
        _target_gas: Option<u64>,
    ) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
        //debug_print!("create target_gas={:?}", target_gas);
        if let Some(depth) = self.state.metadata().depth() {
            if depth + 1 > self.config.call_stack_limit {
                return Capture::Exit((ExitError::CallTooDeep.into(), None, Vec::new()));
            }
        }

        // TODO: check
        // if self.balance(caller) < value {
        //     return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
        // }

        /***
        // This parameter should be true for create from another program
        let take_l64 = true;

        let after_gas = if take_l64 && self.config.call_l64_after_gas {
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
        ***/

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

        Capture::Trap(CreateInterrupt{init_code, context, address})
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

        // These parameters should be true for call from another program
        let take_l64 = true;
        let take_stipend = true;

        /***
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
        ***/

        let hook_res = self.state.call_inner(code_address, transfer, input.clone(), target_gas, is_static, take_l64, take_stipend);
        if hook_res.is_some() {
            match hook_res.as_ref().unwrap() {
                Capture::Exit((reason, return_data)) => {
                    return Capture::Exit((reason.clone(), return_data.clone()))
                },
                Capture::Trap(_interrupt) => {
                    unreachable!("not implemented");
                },
            }
        }

        Capture::Trap(CallInterrupt{code_address, input, context})
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

        debug_print!("Gas used: {}", gas_used(self.state.metadata().gasometer()));
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
        gas_limit: u64,
    ) -> Result<(), ExitError> {
        debug_print!("call_begin gas_limit={}", gas_limit);
        let transaction_cost = gasometer::call_transaction_cost(&input);
        self.executor.state.metadata_mut().gasometer_mut().record_transaction(transaction_cost)?;

        // These parameters should be false for call_begin
        let take_l64 = false;

        let after_gas = if take_l64 && self.executor.config.call_l64_after_gas {
            if self.executor.config.estimate {
                let initial_after_gas = self.executor.state.metadata().gasometer().gas();
                let diff = initial_after_gas - l64(initial_after_gas);
                self.executor.state.metadata_mut().gasometer_mut().record_cost(diff)
                    .map_err(|err| err)?;
                self.executor.state.metadata().gasometer().gas()
            } else {
                l64(self.executor.state.metadata().gasometer().gas())
            }
        } else {
            self.executor.state.metadata().gasometer().gas()
        };

        let gas_limit = core::cmp::min(gas_limit, after_gas);

        self.executor.state.metadata_mut().gasometer_mut().record_cost(gas_limit)
            .map_err(|err| err)?;

        self.executor.state.inc_nonce(caller);

        self.executor.state.enter(gas_limit, false);
        self.executor.state.touch(code_address);

        let code = self.executor.code(code_address);
        let context = evm::Context{address: code_address, caller, apparent_value: U256::zero()};

        let runtime = evm::Runtime::new(code, input, context, self.executor.config);

        self.runtime.push((runtime, CreateReason::Call));

        debug_print!("Gas used: {}", gas_used(self.executor.state.metadata().gasometer()));
        Ok(())
    }

    pub fn create_begin(&mut self,
                        caller: H160,
                        code: Vec<u8>,
                        gas_limit: u64,
    ) -> ProgramResult {
        debug_print!("create_begin gas_limit={}", gas_limit);
        let transaction_cost = gasometer::create_transaction_cost(&code);
        self.executor.state.metadata_mut().gasometer_mut()
            .record_transaction(transaction_cost)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        // This parameter should be false for create_begin
        let take_l64 = false;

        let after_gas = if take_l64 && self.executor.config.call_l64_after_gas {
            if self.executor.config.estimate {
                let initial_after_gas = self.executor.state.metadata().gasometer().gas();
                let diff = initial_after_gas - l64(initial_after_gas);
                self.executor.state.metadata_mut().gasometer_mut().record_cost(diff)
                    .map_err(|_| ProgramError::InvalidInstructionData)?;
                self.executor.state.metadata().gasometer().gas()
            } else {
                l64(self.executor.state.metadata().gasometer().gas())
            }
        } else {
            self.executor.state.metadata().gasometer().gas()
        };

        let gas_limit = core::cmp::min(gas_limit, after_gas);
        self.executor.state.metadata_mut().gasometer_mut().record_cost(gas_limit)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        let scheme = evm::CreateScheme::Legacy { caller };
        self.executor.state.enter(gas_limit, false);

        match self.executor.create(caller, scheme, U256::zero(), code, None) {
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

                let instance = evm::Runtime::new(
                    info.init_code,
                    Vec::new(),
                    info.context,
                    self.executor.config
                );
                self.runtime.push((instance, CreateReason::Create(info.address)));
            },
        }

        debug_print!("Gas used: {}", gas_used(self.executor.state.metadata().gasometer()));
        Ok(())
    }

    fn step_opcode(&mut self) -> RuntimeApply {
        if let Some(runtime) = self.runtime.last_mut() {
            match runtime.0.step(&mut self.executor) {
                Ok(()) => { RuntimeApply::Continue },
                Err(capture) =>
                    match capture {
                        Capture::Exit(reason) => { RuntimeApply::Exit(reason) },
                        Capture::Trap(interrupt) =>
                            match interrupt {
                                Resolve::Call(interrupt, resolve) => {
                                    mem::forget(resolve);
                                    RuntimeApply::Call(interrupt)
                                },
                                Resolve::Create(interrupt, resolve) => {
                                    mem::forget(resolve);
                                    RuntimeApply::Create(interrupt)
                                },
                        }
                }
            }
        }
        else{
            debug_print!("runtime.step: Err, runtime not found");
            RuntimeApply::Exit(ExitReason::Fatal(ExitFatal::NotSupported))
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn step(&mut self) -> Result<(), ExitReason> {
        match self.step_opcode() {
            RuntimeApply::Continue => { Ok(()) },
            RuntimeApply::Call(info) => {
                let code = self.executor.code(info.code_address);
                self.executor.state.enter(u64::MAX, false);
                self.executor.state.touch(info.code_address);

                let instance = evm::Runtime::new(
                    code,
                    info.input,
                    info.context,
                    self.executor.config
                );
                self.runtime.push((instance, CreateReason::Call));
                Ok(())
            },
            RuntimeApply::Create(info) => {
                self.executor.state.enter(u64::MAX, false);
                self.executor.state.touch(info.address);
                self.executor.state.reset_storage(info.address);
                if self.executor.config.create_increase_nonce {
                    self.executor.state.inc_nonce(info.address);
                }

                let instance = evm::Runtime::new(
                    info.init_code,
                    Vec::new(),
                    info.context,
                    self.executor.config
                );
                self.runtime.push((instance, CreateReason::Create(info.address)));
                Ok(())
            },
            RuntimeApply::Exit(exit_reason) => {
                let mut exit_success = false;
                match &exit_reason {
                    ExitReason::Succeed(_) => {
                        exit_success = true;
                        debug_print!(" step_opcode: ExitReason::Succeed(_)");
                        // self.executor.state.exit_commit().unwrap();
                    },
                    ExitReason::Revert(_) => {
                        debug_print!("runtime.step: Err, capture Capture::Exit(reason), reason:ExitReason::Revert(_)");
                        self.executor.state.exit_revert().unwrap();
                    },
                    ExitReason::Error(_) => {
                        debug_print!("runtime.step: Err, capture Capture::Exit(reason), reason:ExitReason::Error(_)");
                        self.executor.state.metadata_mut().gasometer_mut().fail();
                        self.executor.state.exit_discard().unwrap();
                    },
                    ExitReason::Fatal(_) => {
                        debug_print!("runtime.step: Err, capture Capture::Exit(reason), reason:ExitReason::Fatal(_)");
                        self.executor.state.metadata_mut().gasometer_mut().fail();
                        self.executor.state.exit_discard().unwrap();
                    }
                }

                let (return_value, implementation) = {
                    if let Some(runtime) = self.runtime.last(){
                        (runtime.0.machine().return_value(), Some(runtime.1))
                    }
                    else{
                        debug_print!("runtime.step: Err, runtime not found");
                        return Err(ExitReason::Fatal(ExitFatal::NotSupported));
                    }
                };

                match implementation {
                    Some(CreateReason::Call) => {
                        if exit_success {
                            self.executor.state.exit_commit().unwrap();
                        }

                        if self.runtime.len() > 1 {
                            self.runtime.pop();
                            if let Some(runtime) = self.runtime.last_mut(){
                                match  save_return_value(
                                    runtime.0.borrow_mut(),
                                    exit_reason,
                                    return_value,
                                    &self.executor
                                ){
                                    Control::Continue => { Ok(()) },
                                    Control::Exit(e) => { Err(e) },
                                    _ => {
                                        debug_print!("runtime.step: RuntimeApply::Exit, impl::Call, save_return_value: NotSupported");
                                        Err(ExitReason::Fatal(ExitFatal::NotSupported))
                                    }
                                }
                            }
                            else{
                                debug_print!("runtime.step: Err, runtime.last_mut() error");
                                Err(ExitReason::Fatal(ExitFatal::NotSupported))
                            }
                        }
                        else{
                            Err(exit_reason)
                        }

                    },
                    Some(CreateReason::Create(created_address)) => {
                        let mut commit = true;
                        let mut actual_reason = exit_reason;
                        let mut actual_address:Option<H160> = None;

                        if exit_success {
                            if let Some(limit) = self.executor.config.create_contract_limit {
                                if return_value.len() > limit {
                                    debug_print!("runtime.step: Err((ExitError::CreateContractLimit.into()))");
                                    self.executor.state.metadata_mut().gasometer_mut().fail();
                                    self.executor.state.exit_discard().unwrap();
                                    actual_reason =  ExitReason::Error(ExitError::CreateContractLimit);
                                    commit = false;
                                }
                            }
                            if commit{
                                self.executor.state.exit_commit().unwrap();
                                self.executor.state.set_code(created_address, return_value.clone());
                                actual_address = Some(created_address);
                            }
                        }

                        if self.runtime.len() > 1 {
                            self.runtime.pop();
                            if let Some(runtime) = self.runtime.last_mut(){
                                match  save_created_address(
                                    runtime.0.borrow_mut(),
                                    actual_reason,
                                    actual_address,
                                    return_value,
                                    &self.executor
                                ){
                                    Control::Continue => { Ok(()) },
                                    Control::Exit(e) => { Err(e) },
                                    _ => {
                                        debug_print!("runtime.step: RuntimeApply::Exit, impl::Create, save_return_value: NotSupported");
                                        Err(ExitReason::Fatal(ExitFatal::NotSupported))
                                    }
                                }
                            }
                            else{
                                debug_print!("runtime.step: Err, runtime.last_mut() error");
                                Err(ExitReason::Fatal(ExitFatal::NotSupported))
                            }
                        }
                        else{
                            Err(actual_reason)
                        }
                    },
                    _ => {
                        debug_print!("runtime.step: RuntimeApply::Exit, impl: _");
                        Err(ExitReason::Fatal(ExitFatal::NotSupported))
                    }
                }

            },
        }
    }


    pub fn execute(&mut self) -> ExitReason {
        loop {
            if let Err(reason) = self.step() {
                return reason;
            }
        }
    }

    pub fn execute_n_steps(&mut self, n: u64) -> Result<(), ExitReason> {
        for _ in 0..n {
            self.step()?;
        }
        debug_print!(" execute_n_steps OK returned ");

        Ok(())
    }

    #[must_use]
    pub fn return_value(&self) -> Vec<u8> {

        if let Some(runtime) = self.runtime.last() {
            let implementation = Some(runtime.1);
            match implementation {
                Some(CreateReason::Create(_created_address)) => {
                    return Vec::new();
                },
                _ => {
                    return runtime.0.machine().return_value()
                }
            }
        }

        Vec::new()
    }

    pub fn into_state(self) -> ExecutorState<'config, B> {
        self.executor.state
    }
}
