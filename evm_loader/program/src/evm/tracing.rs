use super::{Context, ExitStatus};
use ethnum::U256;

environmental::environmental!(listener: dyn EventListener + 'static);

pub trait EventListener {
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

pub fn with<F: FnOnce(&mut (dyn EventListener + 'static))>(f: F) {
    listener::with(f);
}

pub fn using<R, F: FnOnce() -> R>(
    new: &mut (dyn EventListener + 'static + Send + Sync),
    f: F,
) -> R {
    listener::using(new, f)
}
