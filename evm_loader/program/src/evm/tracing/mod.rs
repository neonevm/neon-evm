use super::{Context, ExitStatus};
use ethnum::U256;

pub mod event_listener;

pub trait EventListener {
    fn enable_return_data(&self) -> bool;
    fn event(&mut self, event: Event);
}

/// Trace event
#[derive(Debug, Clone)]
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
    StackPush {
        value: [u8; 32],
    },
    MemorySet {
        offset: usize,
        data: Vec<u8>,
    },
    StorageSet {
        index: U256,
        value: [u8; 32],
    },
    StorageAccess {
        index: U256,
        value: [u8; 32],
    },
}
