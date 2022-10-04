use borsh::{BorshSerialize, BorshDeserialize};
use evm::{H160, U256, ExitReason, Capture, ExitFatal, Resolve, CONFIG, Control, ExitError, Handler};
use solana_program::{program_error::ProgramError, entrypoint::ProgramResult};

use crate::{
    emit_exit,
    account_storage::AccountStorage
};

use super::{
    handler::{CallInterrupt, CreateInterrupt, Executor}, 
    state::ExecutorState, gasometer::Gasometer, action::Action
};

/// Represents reason of an Ethereum transaction.
/// It can be creation of a smart contract or a call of it's function.
#[derive(BorshSerialize, BorshDeserialize)]
pub enum CreateReason {
    /// Call of a function of smart contract
    Call,
    /// Create (deploy) a smart contract on specified address
    Create(H160),
}

enum RuntimeApply {
    Continue,
    Call(CallInterrupt),
    Create(CreateInterrupt),
    Exit(ExitReason),
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
        let state = ExecutorState::new(backend);
        let gasometer = Gasometer::new(None)?;
        
        let executor = Executor { 
            origin, state, gasometer, 
            gas_limit: U256::zero(), gas_price: U256::zero() 
        };
        Ok(Self { executor, runtime: Vec::new(), steps_executed: 0 })
    }

    /// Serializes and saves state of runtime and executor into a storage account.
    ///
    /// # Panics
    ///
    /// Panics if account is invalid or any serialization error occurs.
    pub fn save_into(&self, storage: &mut crate::account::State) {
        let mut buffer: &mut [u8] = &mut storage.evm_state_mut_data();

        self.runtime.serialize(&mut &mut buffer).unwrap();
        self.executor.state.serialize(&mut &mut buffer).unwrap();
    }

    /// Deserializes and restores state of runtime and executor from a storage account.
    pub fn restore(storage: &crate::account::State, backend: &'a B) -> Result<Self, ProgramError> {
        let mut buffer: &[u8] = &storage.evm_state_data();

        let runtime = BorshDeserialize::deserialize(&mut buffer).unwrap();
        let state = ExecutorState::deserialize(&mut buffer, backend).unwrap();

        let gasometer = Gasometer::new(Some(storage.gas_used))?;
        let executor = Executor { 
            origin: storage.caller,
            state,
            gasometer,
            gas_limit: storage.gas_limit,
            gas_price: storage.gas_price,
        };

        Ok(Self { executor, runtime, steps_executed: 0 })
    }

    /// Begins a call of an Ethereum smart contract.
    ///
    /// # Errors
    ///
    /// May return following errors:
    /// - `InsufficientFunds` if the caller lacks funds for the operation
    pub fn call_begin(
        &mut self,
        caller: H160,
        code_address: H160,
        input: Vec<u8>,
        transfer_value: U256,
        gas_limit: U256,
        gas_price: U256
    ) -> ProgramResult {
        debug_print!("call_begin");

        self.executor.call_begin(caller, code_address, &input, transfer_value, gas_limit, gas_price)?;

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
    pub fn create_begin(
        &mut self,
        caller: H160,
        init_code: Vec<u8>,
        transfer_value: U256,
        gas_limit: U256,
        gas_price: U256
    ) -> ProgramResult {
        debug_print!("create_begin");

        let address = self.executor.create_begin(caller, &init_code, transfer_value, gas_limit, gas_price)?;

        let valids = evm::Valids::compute(&init_code);
        let context = evm::Context{ address, caller, apparent_value: transfer_value };

        let runtime = evm::Runtime::new(init_code, valids, Vec::new(), context);

        self.runtime.push((runtime, CreateReason::Create(address)));

        Ok(())
    }

    #[inline]
    fn process_capture(
        capture: Capture<ExitReason, Resolve<Executor<B>>>,
    ) -> (RuntimeApply, Option<(Vec<u8>, ExitReason)>) {
        match capture {
            Capture::Exit(reason) => {
                if reason == ExitReason::StepLimitReached {
                    (RuntimeApply::Continue, Some((vec![], reason)))
                } else {
                    (RuntimeApply::Exit(reason), Some((vec![], reason)))
                }
            },
            Capture::Trap(interrupt) => {
                match interrupt {
                    Resolve::Call(interrupt, resolve) => {
                        std::mem::forget(resolve);
                        (RuntimeApply::Call(interrupt), None)
                    },
                    Resolve::Create(interrupt, resolve) => {
                        std::mem::forget(resolve);
                        (RuntimeApply::Create(interrupt), None)
                    },
                }
            }
        }
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
                self.state_mut().set_exit_result(Some((vec![], ExitReason::StepLimitReached)));
                return (steps_executed, RuntimeApply::Continue);
            }
            if let Err(capture) = runtime.step(&mut self.executor) {
                let (apply_result, exit_result) = Self::process_capture(capture);

                if exit_result.is_some() {
                    self.state_mut().set_exit_result(exit_result);
                }

                return (steps_executed, apply_result);
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
        let (apply_result, exit_result) = Self::process_capture(capture);

        if exit_result.is_some() {
            self.state_mut().set_exit_result(exit_result);
        }

        (steps_executed, apply_result)
    }

    fn apply_call(&mut self, interrupt: CallInterrupt) -> Result<(), (Vec<u8>, ExitReason)> {
        let code = self.executor.code(interrupt.code_address);
        let valids = self.executor.valids(interrupt.code_address);

        self.executor.state.enter(interrupt.is_static);

        if let Some(transfer) = interrupt.transfer {
            self.executor.transfer(transfer).map_err(|e| (Vec::new(), e.into()))?;
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
        self.executor.state.enter( false);

        if CONFIG.create_increase_nonce {
            self.executor.state.inc_nonce(interrupt.address);
        }

        if let Some(transfer) = interrupt.transfer {
            self.executor.transfer(transfer).map_err(|e| (Vec::new(), e.into()))?;
        }

        let valids = evm::Valids::compute(&interrupt.init_code);
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
            self.executor.state.exit_commit();
        }
        
        let return_value = exited_runtime.machine().return_value();
        if self.runtime.is_empty() {
            return Err((return_value, reason));
        }

        let (runtime, _) = self.runtime.last_mut().unwrap();

        match evm::save_return_value(runtime, reason, return_value, &self.executor) {
            Control::Continue => Ok(()),
            Control::Exit(reason) => Err((Vec::new(), reason)),
            _ => unreachable!()
        }
    }

    fn apply_exit_create(&mut self, exited_runtime: &evm::Runtime, mut reason: ExitReason, address: H160) -> Result<(), (Vec<u8>, ExitReason)> {

        if reason.is_succeed() {
            match CONFIG.create_contract_limit {
                Some(limit) if exited_runtime.machine().return_value_len() > limit => {
                    self.executor.state.exit_revert();
                    reason = ExitError::CreateContractLimit.into();
                },
                _ => {
                    self.executor.state.exit_commit();
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

        match evm::save_created_address(runtime, reason, Some(address), &self.executor) {
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

        if !reason.is_succeed() {
            self.executor.state.exit_revert();
        }

        match create_reason {
            CreateReason::Call => self.apply_exit_call(&exited_runtime, reason),
            CreateReason::Create(address) => self.apply_exit_create(&exited_runtime, reason, address)
        }
    }

    /// Executes current program with all available steps.
    /// # Errors
    /// Terminates execution if a step encounters an error.
    pub fn execute(&mut self) -> (Vec<u8>, ExitReason) {
        loop {
            if let Err(result) = self.execute_n_steps(u64::MAX) {
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
        if let Some(result) = self.state_mut().exit_result() {
            if result.1 != ExitReason::StepLimitReached {
                debug_print!(
                    "Skipping VM execution due to the previous execution result stored to state"
                );
                let result = result.clone();
                self.gasometer_mut().record_additional_resize_iterations(1);
                return Err(result);
            }
        }

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

    /// Consumes Machine and takes gasometer
    #[must_use]
    pub fn take_gasometer(self) -> Gasometer {
        self.executor.gasometer
    }

    /// Returns gasometer mutable reference
    #[must_use]
    pub fn gasometer_mut(&mut self) -> &mut Gasometer {
        &mut self.executor.gasometer
    }

    #[must_use]
    pub fn into_state_actions(self) -> Vec<Action> {
        self.executor.state.into_actions()
    }

    #[must_use]
    pub fn into_state_actions_and_gasometer(self) -> (Vec<Action>, Gasometer) {
        (self.executor.state.into_actions(), self.executor.gasometer)
    }

    #[must_use]
    pub fn state_mut(&mut self) -> &mut ExecutorState<'a, B> {
        &mut self.executor.state
    }
}
