#![allow(clippy::trait_duplication_in_bounds)]
#![allow(clippy::type_repetition_in_bounds)]
#![allow(clippy::unsafe_derive_deserialize)]

use std::marker::PhantomData;

use ethnum::U256;
use serde::{Serialize, Deserialize};

use crate::{
    error::{Error, Result},
    types::{Address, Transaction}, evm::opcode::Action,
};

#[cfg(feature = "tracing")]
pub mod tracing;
pub mod database;
mod memory;
mod opcode;
mod opcode_table;
mod stack;
mod precompile;

use self::{database::Database, memory::Memory, stack::Stack};
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
pub(crate) use tracing_event;

#[derive(Debug, Clone, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum ExitStatus {
    Stop,
    Return(#[serde(with = "serde_bytes")] Vec<u8>),
    Revert(#[serde(with = "serde_bytes")] Vec<u8>),
    Suicide,
    StepLimit,
}

#[derive(Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum Reason {
    Call,
    Create
}

#[derive(Debug, Copy, Clone)]
#[derive(Serialize, Deserialize)]
pub struct Context {
    pub caller: Address,
    pub contract: Address,
    #[serde(with="ethnum::serde::bytes::le")]
    pub value: U256,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "B: Database")]
pub struct Machine<B: Database> {
    origin: Address,
    context: Context,
    
    #[serde(with="ethnum::serde::bytes::le")]
    gas_price: U256,
    #[serde(with="ethnum::serde::bytes::le")]
    gas_limit: U256,
    
    #[serde(with="serde_bytes")]
    execution_code: Vec<u8>,
    #[serde(with="serde_bytes")]
    call_data: Vec<u8>,
    #[serde(with="serde_bytes")]
    return_data: Vec<u8>,
    
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
    pub fn new(
        trx: Transaction,
        origin: Address,
        backend: &mut B,
    ) -> Result<Self> {
        let origin_nonce = backend.nonce(&origin)?;

        if origin_nonce == u64::MAX {
            return Err(Error::NonceOverflow(origin));
        }

        if origin_nonce != trx.nonce {
            return Err(Error::InvalidTransactionNonce(origin, origin_nonce, trx.nonce));
        }

        if let Some(chain_id) = trx.chain_id {
            if backend.chain_id() != chain_id {
                return Err(Error::InvalidChainId(chain_id));
            }
        }

        if backend.balance(&origin)? < trx.value {
            return Err(Error::InsufficientBalanceForTransfer(origin, trx.value));
        }

        if trx.target.is_some() {
            Self::new_call(trx, origin, backend)
        } else {
            Self::new_create(trx, origin, backend)
        }
    }

    fn new_call(
        trx: Transaction,
        origin: Address,
        backend: &mut B,
    ) -> Result<Self> {
        assert!(trx.target.is_some());

        let target = trx.target.unwrap();

        backend.increment_nonce(origin)?;
        backend.snapshot()?;

        backend.transfer(origin, target, trx.value)?;

        let execution_code = backend.code(&target)?;

        Ok(Self {
            origin,
            context: Context { 
                caller: origin,
                contract: target,
                value: trx.value
            },
            gas_price: trx.gas_price,
            gas_limit: trx.gas_limit,
            execution_code,
            call_data: trx.call_data,
            return_data: Vec::new(),
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: false,
            reason: Reason::Call,
            parent: None,
            phantom: PhantomData
        })
    }

    fn new_create(
        trx: Transaction,
        origin: Address,
        backend: &mut B,
    ) -> Result<Self> {
        assert!(trx.target.is_none());

        let target = Address::from_create(&origin, trx.nonce);

        if (backend.nonce(&target)? != 0) || (backend.code_size(&target)? != 0) {
            return Err(Error::DeployToExistingAccount(target, origin));
        }

        backend.increment_nonce(origin)?;
        backend.snapshot()?;

        backend.transfer(origin, target, trx.value)?;

        Ok(Self {
            origin,
            context: Context { 
                caller: origin,
                contract: target,
                value: trx.value
            },
            gas_price: trx.gas_price,
            gas_limit: trx.gas_limit,
            return_data: Vec::new(),
            stack: Stack::new(),
            memory: Memory::with_capacity(trx.call_data.len()),
            pc: 0_usize,
            is_static: false,
            reason: Reason::Create,
            execution_code: trx.call_data,
            call_data: Vec::new(),
            parent: None,
            phantom: PhantomData,
        })
    }

    pub fn execute(&mut self, step_limit: u64, backend: &mut B) -> Result<(ExitStatus, u64)> {
        let mut step = 0_u64;

        tracing_event!(tracing::Event::BeginVM { 
            context: self.context, code: self.execution_code.clone()
        });

        let status = loop {
            step += 1;
            if step > step_limit {
                break ExitStatus::StepLimit;
            }
            
            let opcode = *self.execution_code.get(self.pc).unwrap_or(&0_u8);

            tracing_event!(tracing::Event::BeginStep {
                opcode, pc: self.pc, stack: self.stack.to_vec(), memory: self.memory.to_vec()
            });

            // SAFETY: OPCODES.len() == 256, opcode <= 255
            let opcode_fn = unsafe {
                Self::OPCODES.get_unchecked(opcode as usize)
            };
            let opcode_result = opcode_fn(self, backend)?;

            tracing_event!(opcode_result != Action::Noop; tracing::Event::EndStep {
                gas_used: 0_u64
            });

            match opcode_result {
                Action::Continue => self.pc += 1,
                Action::Jump(target) => self.pc = target,
                Action::Stop => break ExitStatus::Stop,
                Action::Return(value) => break ExitStatus::Return(value),
                Action::Revert(value) => break ExitStatus::Revert(value),
                Action::Suicide => break ExitStatus::Suicide,
                Action::Noop => {},
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
        execution_code: Vec<u8>,
        call_data: Vec<u8>,
        gas_limit: Option<U256>,
    ) {
        let mut other = Self {
            origin: self.origin,
            context,
            gas_price: self.gas_price,
            gas_limit: gas_limit.unwrap_or(self.gas_limit),
            execution_code,
            call_data,
            return_data: Vec::new(),
            stack: Stack::new(),
            memory: Memory::new(),
            pc: 0_usize,
            is_static: self.is_static,
            reason,
            parent: None,
            phantom: PhantomData
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
