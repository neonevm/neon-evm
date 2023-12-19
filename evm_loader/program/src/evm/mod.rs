#![allow(clippy::trait_duplication_in_bounds)]
#![allow(clippy::type_repetition_in_bounds)]
#![allow(clippy::unsafe_derive_deserialize)]

use std::{marker::PhantomData, ops::Range};

use ethnum::U256;
use maybe_async::maybe_async;
use serde::{Deserialize, Serialize};
use solana_program::log::sol_log_data;

pub use buffer::Buffer;

use self::{database::Database, memory::Memory, stack::Stack};
#[cfg(not(target_os = "solana"))]
use crate::evm::tracing::TracerTypeOpt;
use crate::{
    error::{build_revert_message, Error, Result},
    evm::{opcode::Action, precompile::is_precompile_address},
    types::{Address, Transaction},
};

mod analysis;
mod buffer;
pub mod database;
mod eof;
mod memory;
mod opcode;
mod opcode_table;
mod precompile;
mod stack;
#[cfg(not(target_os = "solana"))]
pub mod tracing;
mod utils;
mod validate;

macro_rules! tracing_event {
    ($self:ident, $x:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &$self.tracer {
            tracer.borrow_mut().event($x);
        }
    };
    ($self:ident, $condition:expr, $x:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &$self.tracer {
            if $condition {
                tracer.borrow_mut().event($x);
            }
        }
    };
}

macro_rules! trace_end_step {
    ($self:ident, $return_data:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &$self.tracer {
            tracer
                .borrow_mut()
                .event(crate::evm::tracing::Event::EndStep {
                    gas_used: 0_u64,
                    return_data: $return_data,
                })
        }
    };
    ($self:ident, $condition:expr; $return_data_getter:expr) => {
        #[cfg(not(target_os = "solana"))]
        if $condition {
            trace_end_step!($self, $return_data_getter)
        }
    };
}

use crate::evm::eof::{has_eof_magic, Container};
use crate::evm::opcode::ReturnContext;
pub(crate) use trace_end_step;
pub(crate) use tracing_event;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExitStatus {
    Stop,
    Return(#[serde(with = "serde_bytes")] Vec<u8>),
    Revert(#[serde(with = "serde_bytes")] Vec<u8>),
    Suicide,
    StepLimit,
}

impl ExitStatus {
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self {
            ExitStatus::Return(_) | ExitStatus::Stop | ExitStatus::Suicide => "succeed",
            ExitStatus::Revert(_) => "revert",
            ExitStatus::StepLimit => "step limit exceeded",
        }
    }

    #[must_use]
    pub fn is_succeed(&self) -> Option<bool> {
        match self {
            ExitStatus::Stop | ExitStatus::Return(_) | ExitStatus::Suicide => Some(true),
            ExitStatus::Revert(_) => Some(false),
            ExitStatus::StepLimit => None,
        }
    }

    #[must_use]
    pub fn into_result(self) -> Option<Vec<u8>> {
        match self {
            ExitStatus::Return(v) | ExitStatus::Revert(v) => Some(v),
            ExitStatus::Stop | ExitStatus::Suicide | ExitStatus::StepLimit => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Reason {
    Call,
    Create,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Context {
    pub caller: Address,
    pub contract: Address,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub value: U256,

    pub code_address: Option<Address>,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "B: Database")]
pub struct Machine<B: Database> {
    origin: Address,
    context: Context,

    #[serde(with = "ethnum::serde::bytes::le")]
    gas_price: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    gas_limit: U256,

    execution_code: Buffer,
    container: Option<Container>,
    call_data: Buffer,
    return_data: Buffer,
    return_range: Range<usize>,

    stack: Stack,
    memory: Memory,
    pc: usize,
    code_section: usize,
    return_stack: Vec<ReturnContext>,

    is_static: bool,
    reason: Reason,

    parent: Option<Box<Self>>,

    #[serde(skip)]
    phantom: PhantomData<*const B>,

    #[cfg(not(target_os = "solana"))]
    #[serde(skip)]
    tracer: TracerTypeOpt,
}

impl<B: Database> Machine<B> {
    pub fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize> {
        let mut cursor = std::io::Cursor::new(buffer);

        bincode::serialize_into(&mut cursor, &self)?;

        cursor.position().try_into().map_err(Error::from)
    }

    #[cfg(target_os = "solana")]
    pub fn deserialize_from(buffer: &[u8], backend: &B) -> Result<Self> {
        fn reinit_buffer<B: Database>(buffer: &mut Buffer, backend: &B) {
            if let Some((key, range)) = buffer.uninit_data() {
                *buffer =
                    backend.map_solana_account(&key, |i| unsafe { Buffer::from_account(i, range) });
            }
        }

        fn reinit_machine<B: Database>(mut machine: &mut Machine<B>, backend: &B) {
            loop {
                reinit_buffer(&mut machine.call_data, backend);
                reinit_buffer(&mut machine.execution_code, backend);
                reinit_buffer(&mut machine.return_data, backend);

                if let Some(container) = &mut machine.container {
                    for code in &mut container.code {
                        reinit_buffer(code, backend);
                    }
                    reinit_buffer(&mut container.data, backend);
                }

                match &mut machine.parent {
                    None => break,
                    Some(parent) => machine = parent,
                }
            }
        }

        let mut evm: Self = bincode::deserialize(buffer)?;
        reinit_machine(&mut evm, backend);

        Ok(evm)
    }

    #[maybe_async]
    pub async fn new(
        trx: &mut Transaction,
        origin: Address,
        backend: &mut B,
        #[cfg(not(target_os = "solana"))] tracer: TracerTypeOpt,
    ) -> Result<Self> {
        let origin_nonce = backend.nonce(&origin).await?;

        if origin_nonce == u64::MAX {
            return Err(Error::NonceOverflow(origin));
        }

        if origin_nonce != trx.nonce() {
            return Err(Error::InvalidTransactionNonce(
                origin,
                origin_nonce,
                trx.nonce(),
            ));
        }

        if let Some(chain_id) = trx.chain_id() {
            if backend.chain_id() != chain_id {
                return Err(Error::InvalidChainId(chain_id));
            }
        }

        if backend.balance(&origin).await? < trx.value() {
            return Err(Error::InsufficientBalance(origin, trx.value()));
        }

        if backend.code_size(&origin).await? != 0 {
            return Err(Error::SenderHasDeployedCode(origin));
        }

        if trx.target().is_some() {
            Self::new_call(
                trx,
                origin,
                backend,
                #[cfg(not(target_os = "solana"))]
                tracer,
            )
            .await
        } else {
            Self::new_create(
                trx,
                origin,
                backend,
                #[cfg(not(target_os = "solana"))]
                tracer,
            )
            .await
        }
    }

    #[maybe_async]
    async fn new_call(
        trx: &mut Transaction,
        origin: Address,
        backend: &mut B,
        #[cfg(not(target_os = "solana"))] tracer: TracerTypeOpt,
    ) -> Result<Self> {
        assert!(trx.target().is_some());

        let target = trx.target().unwrap();
        sol_log_data(&[b"ENTER", b"CALL", target.as_bytes()]);

        backend.increment_nonce(origin)?;
        backend.snapshot();

        backend.transfer(origin, target, trx.value()).await?;

        let execution_code = backend.code(&target).await?;

        let container = if has_eof_magic(&execution_code) {
            Some(Container::unmarshal_binary(&execution_code)?)
        } else {
            None
        };

        Ok(Self {
            origin,
            context: Context {
                caller: origin,
                contract: target,
                value: trx.value(),
                code_address: Some(target),
            },
            gas_price: trx.gas_price(),
            gas_limit: trx.gas_limit(),
            execution_code,
            container,
            call_data: trx.extract_call_data(),
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            code_section: 0,
            return_stack: vec![ReturnContext {
                stack_height: 0,
                pc: 0,
                section: 0,
            }],
            is_static: false,
            reason: Reason::Call,
            parent: None,
            phantom: PhantomData,
            #[cfg(not(target_os = "solana"))]
            tracer,
        })
    }

    #[maybe_async]
    async fn new_create(
        trx: &mut Transaction,
        origin: Address,
        backend: &mut B,
        #[cfg(not(target_os = "solana"))] tracer: TracerTypeOpt,
    ) -> Result<Self> {
        assert!(trx.target().is_none());

        let target = Address::from_create(&origin, trx.nonce());
        sol_log_data(&[b"ENTER", b"CREATE", target.as_bytes()]);

        if (backend.nonce(&target).await? != 0) || (backend.code_size(&target).await? != 0) {
            return Err(Error::DeployToExistingAccount(target, origin));
        }

        backend.increment_nonce(origin)?;
        backend.snapshot();

        backend.increment_nonce(target)?;
        backend.transfer(origin, target, trx.value()).await?;

        let code = trx.extract_call_data();

        let container = if has_eof_magic(&code) {
            let container = Container::unmarshal_binary(&code)?;
            container.validate_container()?;
            Some(container)
        } else {
            None
        };

        Ok(Self {
            origin,
            context: Context {
                caller: origin,
                contract: target,
                value: trx.value(),
                code_address: None,
            },
            gas_price: trx.gas_price(),
            gas_limit: trx.gas_limit(),
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            code_section: 0,
            return_stack: vec![ReturnContext {
                stack_height: 0,
                pc: 0,
                section: 0,
            }],
            is_static: false,
            reason: Reason::Create,
            execution_code: code,
            container,
            call_data: Buffer::empty(),
            parent: None,
            phantom: PhantomData,
            #[cfg(not(target_os = "solana"))]
            tracer,
        })
    }

    #[maybe_async]
    pub async fn execute(&mut self, step_limit: u64, backend: &mut B) -> Result<(ExitStatus, u64)> {
        let code = self.get_code();

        assert!(code.is_initialized());
        assert!(self.call_data.is_initialized());
        assert!(self.return_data.is_initialized());

        let mut step = 0_u64;

        tracing_event!(
            self,
            tracing::Event::BeginVM {
                context: self.context,
                code: self.execution_code.to_vec()
            }
        );

        let is_eof = self.container.is_some();

        let status = if is_precompile_address(&self.context.contract) {
            let value = Self::precompile(&self.context.contract, &self.call_data).unwrap();
            backend.commit_snapshot();

            ExitStatus::Return(value)
        } else {
            loop {
                step += 1;
                if step > step_limit {
                    break ExitStatus::StepLimit;
                }

                let code = self.get_code();
                let opcode = code.get_or_default(self.pc);

                tracing_event!(
                    self,
                    tracing::Event::BeginStep {
                        opcode,
                        pc: self.pc,
                        stack: self.stack.to_vec(),
                        memory: self.memory.to_vec()
                    }
                );
                let execution_result = if is_eof {
                    self.execute_eof_opcode(backend, opcode).await
                } else {
                    self.execute_opcode(backend, opcode).await
                };

                let opcode_result = match execution_result {
                    Ok(result) => result,
                    Err(e) => {
                        let message = build_revert_message(&e.to_string());
                        self.opcode_revert_impl(Buffer::from_slice(&message), backend)
                            .await?
                    }
                };

                trace_end_step!(self, opcode_result != Action::Noop; match &opcode_result {
                    Action::Return(value) | Action::Revert(value) => Some(value.clone()),
                    _ => None,
                });

                match opcode_result {
                    Action::Continue => self.pc += 1,
                    Action::Jump(target) => self.pc = target,
                    Action::Stop => break ExitStatus::Stop,
                    Action::Return(value) => break ExitStatus::Return(value),
                    Action::Revert(value) => break ExitStatus::Revert(value),
                    Action::CodeSection(code_section, pc) => {
                        self.code_section = code_section;
                        self.pc = pc;
                    }
                    Action::Suicide => break ExitStatus::Suicide,
                    Action::Noop => {}
                };
            }
        };

        tracing_event!(
            self,
            tracing::Event::EndVM {
                status: status.clone()
            }
        );

        Ok((status, step))
    }

    #[must_use]
    pub fn get_code(&self) -> &Buffer {
        self.container
            .as_ref()
            .map_or(&self.execution_code, |container| {
                &container.code[self.code_section]
            })
    }

    fn fork(
        &mut self,
        reason: Reason,
        context: Context,
        execution_code: Buffer,
        call_data: Buffer,
        gas_limit: Option<U256>,
    ) -> Result<()> {
        let container = if has_eof_magic(&execution_code) {
            let container = Container::unmarshal_binary(&execution_code)?;
            if reason == Reason::Create {
                container.validate_container()?;
            }
            Some(container)
        } else {
            None
        };

        let mut other = Self {
            origin: self.origin,
            context,
            gas_price: self.gas_price,
            gas_limit: gas_limit.unwrap_or(self.gas_limit),
            execution_code,
            container,
            call_data,
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            code_section: 0,
            return_stack: vec![ReturnContext {
                stack_height: 0,
                pc: 0,
                section: 0,
            }],
            is_static: self.is_static,
            reason,
            parent: None,
            phantom: PhantomData,
            #[cfg(not(target_os = "solana"))]
            tracer: self.tracer.clone(),
        };

        core::mem::swap(self, &mut other);
        self.parent = Some(Box::new(other));
        Ok(())
    }

    fn join(&mut self) -> Self {
        assert!(self.parent.is_some());

        let mut other = *self.parent.take().unwrap();
        core::mem::swap(self, &mut other);

        other
    }
}
