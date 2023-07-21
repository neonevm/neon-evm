#![allow(clippy::trait_duplication_in_bounds)]
#![allow(clippy::type_repetition_in_bounds)]
#![allow(clippy::unsafe_derive_deserialize)]

use std::{marker::PhantomData, ops::Range};

use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_program::log::sol_log_data;

use crate::{
    error::{build_revert_message, Error, Result},
    evm::opcode::Action,
    types::{Address, Transaction},
};

mod buffer;
pub mod database;
mod memory;
mod opcode;
mod opcode_table;
mod precompile;
mod stack;
#[cfg(feature = "tracing")]
pub mod tracing;
mod utils;

use self::{database::Database, memory::Memory, stack::Stack};
pub use buffer::Buffer;
pub use precompile::is_precompile_address;

macro_rules! tracing_event {
    ($x:expr) => {
        #[cfg(feature = "tracing")]
        crate::evm::tracing::with(|listener| listener.event($x));
    };
    ($condition:expr; $x:expr) => {
        #[cfg(feature = "tracing")]
        if $condition {
            crate::evm::tracing::with(|listener| listener.event($x));
        }
    };
}

macro_rules! trace_end_step {
    ($return_data_vec:expr) => {
        #[cfg(feature = "tracing")]
        crate::evm::tracing::with(|listener| {
            if listener.enable_return_data() {
                listener.event(crate::evm::tracing::Event::EndStep {
                    gas_used: 0_u64,
                    return_data: $return_data_vec,
                })
            } else {
                listener.event(crate::evm::tracing::Event::EndStep {
                    gas_used: 0_u64,
                    return_data: None,
                })
            }
        })
    };

    ($condition:expr; $return_data_vec:expr) => {
        #[cfg(feature = "tracing")]
        if $condition {
            trace_end_step!($return_data_vec)
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
    call_data: Buffer,
    return_data: Buffer,
    return_range: Range<usize>,

    stack: stack::Stack,
    memory: memory::Memory,
    pc: usize,

    is_static: bool,
    reason: Reason,

    parent: Option<Box<Self>>,

    #[serde(skip)]
    phantom: PhantomData<*const B>,
}

impl<B: Database> Machine<B> {
    pub fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize> {
        let mut cursor = std::io::Cursor::new(buffer);

        bincode::serialize_into(&mut cursor, &self)?;

        cursor.position().try_into().map_err(Error::from)
    }

    pub fn deserialize_from(buffer: &[u8], backend: &B) -> Result<Self> {
        fn reinit_buffer<B: Database>(buffer: &mut Buffer, backend: &B) {
            if let Some((key, range)) = buffer.uninit_data() {
                *buffer = backend.map_solana_account(&key, |i| Buffer::from_account(i, range));
            }
        }

        fn reinit_machine<B: Database>(machine: &mut Machine<B>, backend: &B) {
            reinit_buffer(&mut machine.call_data, backend);
            reinit_buffer(&mut machine.execution_code, backend);
            reinit_buffer(&mut machine.return_data, backend);

            if let Some(parent) = &mut machine.parent {
                reinit_machine(parent, backend);
            }
        }

        let mut evm: Self = bincode::deserialize(buffer)?;
        reinit_machine(&mut evm, backend);

        Ok(evm)
    }

    pub fn new(trx: Transaction, origin: Address, backend: &mut B) -> Result<Self> {
        let origin_nonce = backend.nonce(&origin)?;

        if origin_nonce == u64::MAX {
            return Err(Error::NonceOverflow(origin));
        }

        if origin_nonce != trx.nonce {
            return Err(Error::InvalidTransactionNonce(
                origin,
                origin_nonce,
                trx.nonce,
            ));
        }

        if let Some(chain_id) = trx.chain_id {
            if backend.chain_id() != chain_id {
                return Err(Error::InvalidChainId(chain_id));
            }
        }

        if backend.balance(&origin)? < trx.value {
            return Err(Error::InsufficientBalance(origin, trx.value));
        }

        if backend.code_size(&origin)? != 0 {
            return Err(Error::SenderHasDeployedCode(origin));
        }

        if trx.target.is_some() {
            Self::new_call(trx, origin, backend)
        } else {
            Self::new_create(trx, origin, backend)
        }
    }

    fn new_call(trx: Transaction, origin: Address, backend: &mut B) -> Result<Self> {
        assert!(trx.target.is_some());

        let target = trx.target.unwrap();
        sol_log_data(&[b"ENTER", b"CALL", target.as_bytes()]);

        backend.increment_nonce(origin)?;
        backend.snapshot();

        backend.transfer(origin, target, trx.value)?;

        let execution_code = backend.code(&target)?;

        Ok(Self {
            origin,
            context: Context {
                caller: origin,
                contract: target,
                value: trx.value,
                code_address: Some(target),
            },
            gas_price: trx.gas_price,
            gas_limit: trx.gas_limit,
            execution_code,
            call_data: trx.call_data,
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: false,
            reason: Reason::Call,
            parent: None,
            phantom: PhantomData,
        })
    }

    fn new_create(trx: Transaction, origin: Address, backend: &mut B) -> Result<Self> {
        assert!(trx.target.is_none());

        let target = Address::from_create(&origin, trx.nonce);
        sol_log_data(&[b"ENTER", b"CREATE", target.as_bytes()]);

        if (backend.nonce(&target)? != 0) || (backend.code_size(&target)? != 0) {
            return Err(Error::DeployToExistingAccount(target, origin));
        }

        backend.increment_nonce(origin)?;
        backend.snapshot();

        backend.increment_nonce(target)?;
        backend.transfer(origin, target, trx.value)?;

        Ok(Self {
            origin,
            context: Context {
                caller: origin,
                contract: target,
                value: trx.value,
                code_address: None,
            },
            gas_price: trx.gas_price,
            gas_limit: trx.gas_limit,
            return_data: Buffer::empty(),
            return_range: 0..0,
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: false,
            reason: Reason::Create,
            execution_code: trx.call_data,
            call_data: Buffer::empty(),
            parent: None,
            phantom: PhantomData,
        })
    }

    pub fn execute(&mut self, step_limit: u64, backend: &mut B) -> Result<(ExitStatus, u64)> {
        assert!(self.execution_code.uninit_data().is_none());
        assert!(self.call_data.uninit_data().is_none());
        assert!(self.return_data.uninit_data().is_none());

        let mut step = 0_u64;

        tracing_event!(tracing::Event::BeginVM {
            context: self.context,
            code: self.execution_code.to_vec()
        });

        let status = loop {
            step += 1;
            if step > step_limit {
                break ExitStatus::StepLimit;
            }

            let opcode = self.execution_code.get_or_default(self.pc);

            tracing_event!(tracing::Event::BeginStep {
                opcode,
                pc: self.pc,
                stack: self.stack.to_vec(),
                memory: self.memory.to_vec()
            });

            // SAFETY: OPCODES.len() == 256, opcode <= 255
            let opcode_fn = unsafe { Self::OPCODES.get_unchecked(opcode as usize) };

            let opcode_result = match opcode_fn(self, backend) {
                Ok(result) => result,
                Err(e) => {
                    let message = build_revert_message(&e.to_string());
                    self.opcode_revert_impl(Buffer::from_slice(&message), backend)?
                }
            };

            trace_end_step!(opcode_result != Action::Noop; match &opcode_result {
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
            }
        };

        tracing_event!(tracing::Event::EndVM {
            status: status.clone()
        });

        Ok((status, step))
    }

    fn fork(
        &mut self,
        reason: Reason,
        context: Context,
        execution_code: Buffer,
        call_data: Buffer,
        gas_limit: Option<U256>,
    ) {
        let mut other = Self {
            origin: self.origin,
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
        };

        core::mem::swap(self, &mut other);
        self.parent = Some(Box::new(other));
    }

    fn join(&mut self) -> Self {
        assert!(self.parent.is_some());

        let mut other = *self.parent.take().unwrap();
        core::mem::swap(self, &mut other);

        other
    }
}
