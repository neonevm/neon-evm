#![allow(missing_docs, clippy::missing_panics_doc, clippy::missing_errors_doc)] /// Todo: document

use std::convert::Infallible;
use std::mem;

use evm::{
    Capture, ExitError, ExitFatal, ExitReason,
    H160, H256, Handler, Resolve, Valids, U256,
};
use evm_runtime::{CONFIG, Control, save_created_address, save_return_value};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;

use crate::executor_state::ExecutorState;
use crate::storage_account::StorageAccount;
use crate::utils::{keccak256_h256, keccak256_h256_v};
use crate::precompile_contracts::{call_precompile, is_precompile_address};
use crate::solana_backend::AccountStorage;
use crate::token;
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

struct Executor<'a, B: AccountStorage> {
    state: ExecutorState<'a, B>,
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
        U256::one() //U256::from(self.state.metadata().gasometer().gas())
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
        _target_gas: Option<u64>,
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

        let value = token::eth::round(value);
        if !value.is_zero() && (self.balance(caller) < value) {
            return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
        }

        // Get the create address from given scheme.
        let address = self.create_address(scheme);

        event!(Create {
            caller,
            address,
            scheme,
            value,
            init_code: &init_code,
            _target_gas,
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
        _target_gas: Option<u64>,
        is_static: bool,
        context: evm::Context,
    ) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
        event!(Call {
            code_address,
            transfer: &transfer,
            input: &input,
            _target_gas,
            is_static,
            context: &context,
        });

        debug_print!("call");

        if (self.state.metadata().is_static() || is_static) && transfer.is_some() {
            return Capture::Exit((ExitError::StaticModeViolation.into(), Vec::new()))
        }

        let transfer = transfer.map(|t| {
            evm::Transfer { source: t.source, target: t.target, value: token::eth::round(t.value) }
        });
        let context = evm::Context {
            address: context.address,
            caller: context.caller,
            apparent_value: token::eth::round(context.apparent_value)
        };

        let precompile_result = call_precompile(code_address, &input, &context, &mut self.state);
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum CreateReason {
    Call,
    Create(H160),
}

type RuntimeInfo = (evm::Runtime, CreateReason);

pub struct Machine<'a, B: AccountStorage> {
    executor: Executor<'a, B>,
    runtime: Vec<RuntimeInfo>,
    steps_executed: u64,
}

impl<'a, B: AccountStorage> Machine<'a, B> {
    #[must_use]
    pub fn new(state: ExecutorState<'a, B>) -> Self {
        let executor = Executor { state };
        Self{ executor, runtime: Vec::new(), steps_executed: 0 }
    }

    pub fn save_into(&self, storage: &mut StorageAccount) {
        storage.serialize(&self.runtime, self.executor.state.substate()).unwrap();
    }

    #[must_use]
    pub fn restore(storage: &StorageAccount, backend: &'a B) -> Self {
        let (runtime, substate) = storage.deserialize().unwrap();

        let state = ExecutorState::new(substate, backend);

        let executor = Executor { state };
        Self{ executor, runtime, steps_executed: 0 }
    }

    pub fn call_begin(&mut self,
        caller: H160,
        code_address: H160,
        input: Vec<u8>,
        transfer_value: U256,
        _gas_limit: u64
    ) -> ProgramResult {
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

        let transfer_value = token::eth::round(transfer_value);
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

    pub fn create_begin(&mut self,
                        caller: H160,
                        code: Vec<u8>,
                        transfer_value: U256,
                        _gas_limit: u64,
    ) -> ProgramResult {
        event!(TransactCreate {
            caller,
            value: transfer_value,
            init_code: &code,
            _gas_limit,
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
            self.steps_executed += steps_executed;

            match apply {
                RuntimeApply::Continue => (),
                RuntimeApply::Call(info) => self.apply_call(info)?,
                RuntimeApply::Create(info) => self.apply_create(info)?,
                RuntimeApply::Exit(reason) => self.apply_exit(reason)?,
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn get_steps_executed(&self) -> u64 {
        self.steps_executed
    }

    #[must_use]
    pub fn into_state(self) -> ExecutorState<'a, B> {
        self.executor.state
    }
}
