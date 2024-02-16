use super::{Context, ExitStatus};
use crate::evm::database::Database;
use ethnum::U256;

pub struct NoopEventListener;

pub trait EventListener {
    fn event(&mut self, executor_state: &impl Database, event: Event);
}

impl EventListener for NoopEventListener {
    fn event(&mut self, _executor_state: &impl Database, _event: Event) {}
}

/// Trace event
pub enum Event {
    BeginVM {
        context: Context,
        code: Vec<u8>,
    },
    EndVM {
        status: ExitStatus,
    },
    BeginStep {
        opcode: u8,
        pc: usize,
        stack: Vec<[u8; 32]>,
        memory: Vec<u8>,
    },
    EndStep {
        gas_used: u64,
        return_data: Option<Vec<u8>>,
    },
    StorageAccess {
        index: U256,
        value: [u8; 32],
    },
}
