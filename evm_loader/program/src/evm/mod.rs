#![allow(clippy::trait_duplication_in_bounds)]
#![allow(clippy::type_repetition_in_bounds)]
#![allow(clippy::unsafe_derive_deserialize)]

use std::{fmt::Display, marker::PhantomData, ops::Range};

use ethnum::U256;
use maybe_async::maybe_async;
use serde::{Deserialize, Serialize};

pub use buffer::Buffer;

use crate::{evm::tracing::EventListener, types::boxx::Boxx};
#[cfg(target_os = "solana")]
use crate::evm::tracing::NoopEventListener;
use crate::{
    debug::log_data,
    error::{build_revert_message, Error, Result},
    evm::{opcode::Action, precompile::is_precompile_address},
    types::{Address, Transaction},
};

use self::{database::Database, memory::Memory, stack::Stack};

mod buffer;
pub mod database;
mod memory;
mod opcode;
pub mod opcode_table;
mod precompile;
mod stack;
pub mod tracing;
mod utils;

macro_rules! tracing_event {
    ($self:ident, $backend:ident, $x:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &mut $self.tracer {
            tracer.event($backend, $x);
        }
    };
    ($self:ident, $backend:ident, $condition:expr, $x:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &mut $self.tracer {
            if $condition {
                tracer.event($backend, $x);
            }
        }
    };
}

macro_rules! trace_end_step {
    ($self:ident, $backend:ident, $return_data:expr) => {
        #[cfg(not(target_os = "solana"))]
        if let Some(tracer) = &mut $self.tracer {
            tracer.event(
                $backend,
                crate::evm::tracing::Event::EndStep {
                    gas_used: 0_u64,
                    return_data: $return_data,
                },
            )
        }
    };
    ($self:ident, $backend:ident, $condition:expr; $return_data_getter:expr) => {
        #[cfg(not(target_os = "solana"))]
        if $condition {
            trace_end_step!($self, $backend, $return_data_getter)
        }
    };
}

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

impl Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.status())
    }
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
    pub contract_chain_id: u64,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub value: U256,

    pub code_address: Option<Address>,
}

pub struct Machine<B: Database, T: EventListener> {
    origin: Address,
    chain_id: u64,
    context: Context,

    gas_price: U256,
    gas_limit: U256,

    execution_code: Buffer,
    call_data: Buffer,
    return_data: Buffer,
    return_range: Range<usize>,

    stack: Stack,
    memory: Memory,
    pc: usize,

    is_static: bool,
    reason: Reason,

    parent: Option<Boxx<Self>>,

    phantom: PhantomData<*const B>,

    tracer: Option<T>,
}

#[cfg(target_os = "solana")]
impl<B: Database> Machine<B, NoopEventListener> {
    fn reinit_buffer(buffer: &mut Buffer, backend: &B) {
        if let Some((key, range)) = buffer.uninit_data() {
            *buffer =
                backend.map_solana_account(&key, |i| unsafe { Buffer::from_account(i, range) });
        }
    }

    pub fn reinit(&mut self, backend: &B) {
        let mut machine = self;
        loop {
            Self::reinit_buffer(&mut machine.call_data, backend);
            Self::reinit_buffer(&mut machine.execution_code, backend);
            Self::reinit_buffer(&mut machine.return_data, backend);

            match &mut machine.parent {
                None => break,
                Some(parent) => machine = parent,
            }
        }
    }
}

impl<B: Database, T: EventListener> Machine<B, T> {
    #[maybe_async]
    pub async fn new(
        trx: &Transaction,
        origin: Address,
        backend: &mut B,
        tracer: Option<T>,
    ) -> Result<Self> {
        let trx_chain_id = trx.chain_id().unwrap_or_else(|| backend.default_chain_id());

        if backend.balance(origin, trx_chain_id).await? < trx.value() {
            return Err(Error::InsufficientBalance(
                origin,
                trx_chain_id,
                trx.value(),
            ));
        }

        if trx.target().is_some() {
            Self::new_call(trx_chain_id, trx, origin, backend, tracer).await
        } else {
            Self::new_create(trx_chain_id, trx, origin, backend, tracer).await
        }
    }

    #[maybe_async]
    async fn new_call(
        chain_id: u64,
        trx: &Transaction,
        origin: Address,
        backend: &mut B,
        tracer: Option<T>,
    ) -> Result<Self> {
        assert!(trx.target().is_some());

        let target = trx.target().unwrap();
        log_data(&[b"ENTER", b"CALL", target.as_bytes()]);

        backend.snapshot();

        backend
            .transfer(origin, target, chain_id, trx.value())
            .await?;

        let execution_code = backend.code(target).await?;

        Ok(Self {
            origin,
            chain_id,
            context: Context {
                caller: origin,
                contract: target,
                contract_chain_id: backend.contract_chain_id(target).await.unwrap_or(chain_id),
                value: trx.value(),
                code_address: Some(target),
            },
            gas_price: trx.gas_price(),
            gas_limit: trx.gas_limit(),
            execution_code,
            call_data: Buffer::from_slice(trx.call_data()),
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: false,
            reason: Reason::Call,
            parent: None,
            phantom: PhantomData,
            tracer,
        })
    }

    #[maybe_async]
    async fn new_create(
        chain_id: u64,
        trx: &Transaction,
        origin: Address,
        backend: &mut B,
        tracer: Option<T>,
    ) -> Result<Self> {
        assert!(trx.target().is_none());

        let target = Address::from_create(&origin, trx.nonce());
        log_data(&[b"ENTER", b"CREATE", target.as_bytes()]);

        if (backend.nonce(target, chain_id).await? != 0) || (backend.code_size(target).await? != 0)
        {
            return Err(Error::DeployToExistingAccount(target, origin));
        }

        backend.snapshot();

        backend.increment_nonce(target, chain_id)?;
        backend
            .transfer(origin, target, chain_id, trx.value())
            .await?;

        Ok(Self {
            origin,
            chain_id,
            context: Context {
                caller: origin,
                contract: target,
                contract_chain_id: chain_id,
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
            is_static: false,
            reason: Reason::Create,
            execution_code: Buffer::from_slice(trx.call_data()),
            call_data: Buffer::empty(),
            parent: None,
            phantom: PhantomData,
            tracer,
        })
    }

    #[maybe_async]
    pub async fn execute(
        &mut self,
        step_limit: u64,
        backend: &mut B,
    ) -> Result<(ExitStatus, u64, Option<T>)> {
        assert!(self.execution_code.is_initialized());
        assert!(self.call_data.is_initialized());
        assert!(self.return_data.is_initialized());

        let mut step = 0_u64;

        tracing_event!(
            self,
            backend,
            tracing::Event::BeginVM {
                context: self.context,
                code: self.execution_code.to_vec()
            }
        );

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

                let opcode = self.execution_code.get_or_default(self.pc);

                tracing_event!(
                    self,
                    backend,
                    tracing::Event::BeginStep {
                        opcode,
                        pc: self.pc,
                        stack: self.stack.to_vec(),
                        memory: self.memory.to_vec()
                    }
                );

                let opcode_result = match self.execute_opcode(backend, opcode).await {
                    Ok(result) => result,
                    Err(e) => {
                        let message = build_revert_message(&e.to_string());
                        self.opcode_revert_impl(message, backend).await?
                    }
                };

                trace_end_step!(self, backend, opcode_result != Action::Noop; match &opcode_result {
                    Action::Return(value) | Action::Revert(value) => Some(value.clone()),
                    _ => None,
                });

                match opcode_result {
                    Action::Continue => self.pc += 1,
                    Action::Jump(target) => self.pc = target,
                    Action::Stop => break ExitStatus::Stop,
                    Action::Return(value) => break ExitStatus::Return(value),
                    Action::Revert(value) => break ExitStatus::Revert(value),
                    Action::Suicide => break ExitStatus::Suicide,
                    Action::Noop => {}
                };
            }
        };

        tracing_event!(
            self,
            backend,
            tracing::Event::EndVM {
                status: status.clone()
            }
        );

        Ok((status, step, self.tracer.take()))
    }

    fn fork(
        &mut self,
        reason: Reason,
        chain_id: u64,
        context: Context,
        execution_code: Buffer,
        call_data: Buffer,
        gas_limit: Option<U256>,
    ) {
        let mut other = Self {
            origin: self.origin,
            chain_id,
            context,
            gas_price: self.gas_price,
            gas_limit: gas_limit.unwrap_or(self.gas_limit),
            execution_code,
            call_data,
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: self.is_static,
            reason,
            parent: None,
            phantom: PhantomData,
            tracer: self.tracer.take(),
        };

        core::mem::swap(self, &mut other);
        self.parent = Some(crate::types::boxx::boxx(other));
    }

    fn join(&mut self) -> Self {
        assert!(self.parent.is_some());

        let mut other = Boxx::into_inner(self.parent.take().unwrap());
        core::mem::swap(self, &mut other);

        self.tracer = other.tracer.take();

        other
    }
}
