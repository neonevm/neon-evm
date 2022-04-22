//! # Neon EVM Executor
//!
//! Executor is a struct that hooks gasometer and the EVM core together.
//! It also handles the call stacks in EVM.

use std::convert::Infallible;
use std::mem;
use std::boxed::Box;

use evm::{
    Capture, ExitError, ExitFatal, ExitReason,
    H160, H256, Handler, Resolve, Valids, U256,
};
use evm_runtime::{CONFIG, Control, save_created_address, save_return_value};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use crate::executor_state::{ExecutorState, ExecutorSubstate};
use crate::utils::{keccak256_h256, keccak256_h256_v};
use crate::precompile_contracts::{call_precompile, is_precompile_address};
use crate::account_storage::AccountStorage;
use crate::gasometer::Gasometer;
use crate::{event, emit_exit};


fn emit_exit<E: Into<ExitReason> + Copy>(error: E) -> E {
    emit_exit!(error)
}

/// "All but one 64th" operation.
/// See also EIP-150.

#[derive(Debug)]
struct CallInterrupt {
    context: evm::Context,
    transfer: Option<evm::Transfer>,
    code_address: H160,
    input: Vec<u8>,
    is_static: bool,
}

#[derive(Debug)]
struct CreateInterrupt {
    context: evm::Context,
    transfer: Option<evm::Transfer>,
    address: H160,
    init_code: Vec<u8>,
}

#[derive(Debug)]
enum RuntimeApply{
    Continue,
    Call(CallInterrupt),
    Create(CreateInterrupt),
    Exit(ExitReason),
}

/// Stack-based executor.
struct Executor<'a, B: AccountStorage> {
    origin: H160,
    state: ExecutorState<'a, B>,
    gasometer: Gasometer
}

impl<'a, B: AccountStorage> Executor<'a, B> {
    fn create_address(&self, scheme: evm::CreateScheme) -> H160 {
        match scheme {
            evm::CreateScheme::Create2 { caller, code_hash, salt } => {
                keccak256_h256_v(&[&[0xff], &caller[..], &salt[..], &code_hash[..]]).into()
            },
            evm::CreateScheme::Legacy { caller } => {
                let nonce = self.state.nonce(caller);
                let mut stream = rlp::RlpStream::new_list(2);
                stream.append(&caller);
                stream.append(&nonce);
                keccak256_h256(&stream.out()).into()
            },
            evm::CreateScheme::Fixed(naddress) => {
                naddress
            },
        }
    }
}

impl<'a, B: AccountStorage> Handler for Executor<'a, B> {
    type CreateInterrupt = crate::executor::CreateInterrupt;
    type CreateFeedback = Infallible;
    type CallInterrupt = crate::executor::CallInterrupt;
    type CallFeedback = Infallible;

    fn keccak256_h256(&self, data: &[u8]) -> H256 {
        keccak256_h256(data)
    }

    fn balance(&self, address: H160) -> U256 {
        self.state.balance(address)
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
        U256::one() // TODO
    }

    fn gas_price(&self) -> U256 {
        U256::zero() // TODO
    }

    fn origin(&self) -> H160 {
        self.origin
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
        if is_precompile_address(&address) {
            return true;
        }
        
        if CONFIG.empty_considered_exists {
            self.state.exists(address)
        } else {
            self.state.exists(address) && !self.state.is_empty(address)
        }
    }

    fn deleted(&self, address: H160) -> bool {
        self.state.deleted(address)
    }

    fn set_storage(&mut self, address: H160, index: U256, value: U256) -> Result<(), ExitError> {
        if self.state.metadata().is_static() {
            return Err(ExitError::StaticModeViolation);
        }

        self.gasometer.record_storage_write(&self.state, address, index);

        self.state.set_storage(address, index, value);
        Ok(())
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
        if self.state.metadata().is_static() {
            return Err(ExitError::StaticModeViolation);
        }

        self.state.log(address, topics, data);
        Ok(())
    }

    fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
        if self.state.metadata().is_static() {
            return Err(ExitError::StaticModeViolation);
        }

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
        debug_print!("create");

        if self.state.metadata().is_static() {
            return Capture::Exit((ExitError::StaticModeViolation.into(), None, Vec::new()))
        }

        if let Some(depth) = self.state.metadata().depth() {
            if depth + 1 > CONFIG.call_stack_limit {
                return Capture::Exit((ExitError::CallTooDeep.into(), None, Vec::new()));
            }
        }

        if !value.is_zero() && (self.balance(caller) < value) {
            return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
        }

        // Get the create address from given scheme.
        let address = self.create_address(scheme);
        let _ = target_gas;
        event!(Create {
            caller,
            address,
            scheme,
            value,
            init_code: &init_code,
            target_gas,
        });


        // TODO: may be increment caller's nonce after runtime creation or success execution?
        self.state.inc_nonce(caller);

        let existing_code = self.state.code(address);
        if !existing_code.is_empty() {
            // let _ = self.merge_fail(substate);
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        if self.state.nonce(address)  > U256::zero() {
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        let context = evm::Context {
            address,
            caller,
            apparent_value: value,
        };

        let transfer = Some(evm::Transfer { source: caller, target: address, value });

        Capture::Trap(CreateInterrupt{context, transfer, address, init_code})
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
        let _ = target_gas;
        event!(Call {
            code_address,
            transfer: &transfer,
            input: &input,
            target_gas,
            is_static,
            context: &context,
        });

        debug_print!("call");

        if (self.state.metadata().is_static() || is_static) && transfer.is_some() {
            return Capture::Exit((ExitError::StaticModeViolation.into(), Vec::new()))
        }

        let precompile_result = call_precompile(code_address, &input, &context, &mut self.state, &mut self.gasometer);
        if let Some(Capture::Exit(exit_value)) = precompile_result {
            return Capture::Exit(exit_value);
        }

        if let Some(depth) = self.state.metadata().depth() {
            if depth + 1 > CONFIG.call_stack_limit {
                return Capture::Exit((ExitError::CallTooDeep.into(), Vec::new()));
            }
        }

        Capture::Trap(CallInterrupt{context, transfer, code_address, input, is_static})
    }

    fn pre_validate(
        &mut self,
        _context: &evm::Context,
        _opcode: evm::Opcode,
        _stack: &evm::Stack,
    ) -> Result<(), ExitError> {
        Ok(())
    }
}

/// Represents reason of an Ethereum transaction.
/// It can be creation of a smart contract or a call of it's function.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum CreateReason {
    /// Call of a function of smart contract
    Call,
    /// Create (deploy) a smart contract on specified address
    Create(H160),
}

type RuntimeInfo = (evm::Runtime, CreateReason);

/// Represents a virtual machine.
pub struct Machine<'a, B: AccountStorage> {
    executor: Executor<'a, B>,
    runtime: Vec<RuntimeInfo>,
    steps_executed: u64,
}

impl<'a, B: AccountStorage> Machine<'a, B> {
    /// Creates instance of the Machine.
    pub fn new(origin: H160, backend: &'a B) -> Result<Self, ProgramError> {
        let substate = Box::new(ExecutorSubstate::new(backend));
        let state = ExecutorState::new(substate, backend);
        let gasometer = Gasometer::new()?;
        
        let executor = Executor { origin, state, gasometer };
        Ok(Self { executor, runtime: Vec::new(), steps_executed: 0 })
    }

    /// Serializes and saves state of runtime and executor into a storage account.
    ///
    /// # Panics
    ///
    /// Panics if account is invalid or any serialization error occurs.
    pub fn save_into(&self, storage: &mut crate::account::Storage) {
        storage.serialize(&self.runtime, self.executor.state.substate()).unwrap();
    }

    /// Deserializes and restores state of runtime and executor from a storage account.
    pub fn restore(storage: &crate::account::Storage, backend: &'a B) -> Result<Self, ProgramError> {
        let (runtime, substate) = storage.deserialize()?;
        let gasometer = Gasometer::new()?;

        let origin = storage.caller;
        let state = ExecutorState::new(substate, backend);

        let executor = Executor { origin, state, gasometer };
        Ok(Self { executor, runtime, steps_executed: 0 })
    }

    /// Begins a call of an Ethereum smart contract.
    ///
    /// # Errors
    ///
    /// May return following errors:
    /// - `InsufficientFunds` if the caller lacks funds for the operation
    pub fn call_begin(&mut self,
        caller: H160,
        code_address: H160,
        input: Vec<u8>,
        transfer_value: U256,
        gas_limit: U256
    ) -> ProgramResult {
	    let _ = gas_limit;
        event!(TransactCall {
            caller,
            address: code_address,
            value: transfer_value,
            data: &input,
            gas_limit
        });
        debug_print!("call_begin");

        self.executor.state.inc_nonce(caller);
        self.executor.state.enter(false);
        self.executor.state.touch(code_address);

        self.executor.gasometer.record_transfer(&self.executor.state, code_address, transfer_value);

        let transfer = evm::Transfer { source: caller, target: code_address, value: transfer_value };
        self.executor.state.transfer(&transfer)
            .map_err(emit_exit)
            .map_err(|e| E!(ProgramError::InsufficientFunds; "ExitError={:?}", e))?;

        let code = self.executor.code(code_address);
        let valids = self.executor.valids(code_address);
        let context = evm::Context{ address: code_address, caller, apparent_value: transfer_value };

        let runtime = evm::Runtime::new(code, valids, input, context);

        self.runtime.push((runtime, CreateReason::Call));

        Ok(())
    }

    /// Begins a creation (deployment) of an Ethereum smart contract.
    ///
    /// # Errors
    ///
    /// May return following errors:
    /// - `InsufficientFunds` if the caller lacks funds for the operation
    pub fn create_begin(&mut self,
                        caller: H160,
                        code: Vec<u8>,
                        transfer_value: U256,
                        gas_limit: U256,
    ) -> ProgramResult {
        let _ = gas_limit;
        event!(TransactCreate {
            caller,
            value: transfer_value,
            init_code: &code,
            gas_limit,
            address: self.executor.create_address(evm::CreateScheme::Legacy { caller }),
        });

        debug_print!("create_begin");

        let scheme = evm::CreateScheme::Legacy { caller };

        match self.executor.create(caller, scheme, transfer_value, code, None) {
            Capture::Exit((reason, addr, value)) => {
                let (value, reason) = emit_exit!(value, reason);
                return Err!(ProgramError::InvalidInstructionData; "create_begin() error={:?} ", (reason, addr, value));
            },
            Capture::Trap(info) => {
                self.executor.state.enter(false);

                self.executor.state.touch(info.address);
                self.executor.state.reset_storage(info.address);
                if CONFIG.create_increase_nonce {
                    self.executor.state.inc_nonce(info.address);
                }

                self.executor.gasometer.record_deploy(&self.executor.state, info.address);

                if let Some(transfer) = info.transfer {
                    self.executor.state.transfer(&transfer)
                        .map_err(emit_exit)
                        .map_err(|e| E!(ProgramError::InsufficientFunds; "ExitError={:?}", e))?;
                }

                let valids = Valids::compute(&info.init_code);
                let instance = evm::Runtime::new(
                    info.init_code,
                    valids,
                    Vec::new(),
                    info.context,
                );

                self.runtime.push((instance, CreateReason::Create(info.address)));
            },
        }

        Ok(())
    }

    #[cfg(feature = "tracing")]
    fn run(&mut self, max_steps: u64) -> (u64, RuntimeApply) {
        let runtime = match self.runtime.last_mut() {
            Some((runtime, _)) => runtime,
            None => return (0, RuntimeApply::Exit(ExitFatal::NotSupported.into()))
        };

        let mut steps_executed = 0;
        loop {
            if steps_executed >= max_steps {
                    return (steps_executed, RuntimeApply::Continue);
            }
            if let Err(capture) = runtime.step(&mut self.executor) {
                return match capture {
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
                };
            }
            steps_executed += 1;
        }
    }

    #[cfg(not(feature = "tracing"))]
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

        self.executor.state.enter(interrupt.is_static);
        self.executor.state.touch(interrupt.code_address);

        if let Some(transfer) = interrupt.transfer {
            self.executor.gasometer.record_transfer(&self.executor.state, interrupt.code_address, transfer.value);
            self.executor.state.transfer(&transfer).map_err(|_| (Vec::new(), ExitError::OutOfFund.into()))?;
        }

        let instance = evm::Runtime::new(
            code,
            valids,
            interrupt.input,
            interrupt.context,
        );
        self.runtime.push((instance, CreateReason::Call));

        Ok(())
    }

    fn apply_create(&mut self, interrupt: CreateInterrupt) -> Result<(), (Vec<u8>, ExitReason)> {
        debug_print!("apply_create {:?}", interrupt);
        self.executor.state.enter( false);
        self.executor.state.touch(interrupt.address);
        self.executor.state.reset_storage(interrupt.address);
        if CONFIG.create_increase_nonce {
            self.executor.state.inc_nonce(interrupt.address);
        }

        self.executor.gasometer.record_deploy(&self.executor.state, interrupt.address);

        if let Some(transfer) = interrupt.transfer {
            self.executor.state.transfer(&transfer).map_err(|_| (Vec::new(), ExitError::OutOfFund.into()))?;
        }

        let valids = Valids::compute(&interrupt.init_code);
        let instance = evm::Runtime::new(
            interrupt.init_code,
            valids,
            Vec::new(),
            interrupt.context,
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

        if reason.is_succeed() {
            match CONFIG.create_contract_limit {
                Some(limit) if exited_runtime.machine().return_value_len() > limit => {
                    self.executor.state.exit_discard().map_err(|e| (Vec::new(), ExitReason::from(e)))?;
                    reason = ExitError::CreateContractLimit.into();
                },
                _ => {
                    self.executor.state.exit_commit().map_err(|e| (Vec::new(), ExitReason::from(e)))?;
                    let return_value = exited_runtime.machine().return_value();
                    self.executor.state.set_code(address, return_value);
                }
            };
        }

        let runtime = match self.runtime.last_mut() {
            Some((runtime, _)) => runtime,
            None => return match reason {
                ExitReason::Revert(_) => {
                    let return_value = exited_runtime.machine().return_value();
                    Err((return_value, reason))
                },
                _ => Err((Vec::<u8>::new(), reason))
            }
        };

        match save_created_address(runtime, reason, Some(address), &self.executor) {
            Control::Continue => Ok(()),
            Control::Exit(reason) => Err((Vec::new(), reason)),
            _ => unreachable!()
        }
    }

    fn apply_exit(&mut self, reason: ExitReason) -> Result<(), (Vec<u8>, ExitReason)> {
        let (exited_runtime, create_reason) = match self.runtime.pop() {
            Some((runtime, reason)) => (runtime, reason),
            None => return Err((Vec::new(), ExitFatal::NotSupported.into()))
        };

        emit_exit!(exited_runtime.machine().return_value(), reason);

        match reason {
            ExitReason::Succeed(_) => Ok(()),
            ExitReason::Revert(_) => self.executor.state.exit_revert(),
            ExitReason::Error(_) | ExitReason::Fatal(_) => self.executor.state.exit_discard(),
            ExitReason::StepLimitReached => unreachable!()
        }.map_err(|e| (exited_runtime.machine().return_value(), ExitReason::from(e)))?;

        match create_reason {
            CreateReason::Call => self.apply_exit_call(&exited_runtime, reason),
            CreateReason::Create(address) => self.apply_exit_create(&exited_runtime, reason, address)
        }
    }

    /// Executes current program with all available steps.
    /// # Errors
    /// Terminates execution if a step encounteres an error.
    pub fn execute(&mut self) -> (Vec<u8>, ExitReason) {
        loop {
            if let Err(result) = self.execute_n_steps(u64::max_value()) {
                return result;
            }
        }
    }

    /// Executes up to `n` steps of current path of execution.
    ///
    /// # Errors
    ///
    /// Execution may return following exit reasons:
    /// - `StepLimitReached` if reached a step limit
    /// - `Succeed` if has succeeded
    /// - `Error` if returns a normal EVM error
    /// - `Revert` if encountered an explicit revert
    /// - `Fatal` if encountered an error that is not supposed to be normal EVM errors
    pub fn execute_n_steps(&mut self, n: u64) -> Result<(), (Vec<u8>, ExitReason)> {
        let mut steps = 0_u64;

        while steps < n {
            let (steps_executed, apply) = self.run(n - steps);
            steps += steps_executed;

            self.steps_executed += steps_executed;
            self.executor.gasometer.record_evm_steps(steps_executed);

            match apply {
                RuntimeApply::Continue => (),
                RuntimeApply::Call(info) => self.apply_call(info)?,
                RuntimeApply::Create(info) => self.apply_create(info)?,
                RuntimeApply::Exit(reason) => self.apply_exit(reason)?,
            }
        }

        Ok(())
    }

    /// Returns number of executed steps.
    #[must_use]
    pub fn get_steps_executed(&self) -> u64 {
        self.steps_executed
    }

    /// Returns amount of used gas
    #[must_use]
    pub fn used_gas(&self) -> U256 {
        self.executor.gasometer.used_gas()
    }

    /// Returns gasometer mutable reference
    #[must_use]
    pub fn gasometer_mut(&mut self) -> &mut Gasometer {
        &mut self.executor.gasometer
    }

    #[must_use]
    pub fn into_state(self) -> ExecutorState<'a, B> {
        self.executor.state
    }
}
