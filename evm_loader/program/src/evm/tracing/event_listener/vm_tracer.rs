use crate::evm::tracing::event_listener::trace::{ExecutiveVMTracer, MemoryDiff, StorageDiff};
use ethnum::U256;

#[derive(Debug, Default, Clone)]
pub struct StepDiff {
    pub storage_access: Option<(U256, [u8; 32])>,
    pub storage_set: Option<StorageDiff>,
    pub memory_set: Option<MemoryDiff>,
    pub stack_push: Vec<[u8; 32]>,
}

pub struct VmTracer {
    pub tracer: ExecutiveVMTracer,
    step_diff: Vec<StepDiff>,
}

impl VmTracer {
    pub fn init() -> Self {
        VmTracer {
            tracer: ExecutiveVMTracer::toplevel(),
            step_diff: Vec::new(),
        }
    }

    pub fn push_step_diff(&mut self) {
        self.step_diff.push(StepDiff::default());
    }

    pub fn pop_step_diff(&mut self) {
        self.step_diff.pop();
    }

    pub fn step_diff(&mut self) -> &mut StepDiff {
        self.step_diff
            .last_mut()
            .expect("diff was pushed in begin_vm")
    }
}
