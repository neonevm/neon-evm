use std::sync::{Arc, RwLock};

use crate::evm::tracing::event_listener::trace::{FullTraceData, VMTrace, VMTracer};

use super::vm_tracer::VmTracer;

pub type TracerType = Option<Arc<RwLock<Option<Tracer>>>>;

pub struct Tracer {
    pub vm: VmTracer,
    pub data: Vec<FullTraceData>,
    pub enable_return_data: bool,
}

impl Tracer {
    #[must_use]
    pub fn new(enable_return_data: bool) -> Self {
        Tracer {
            vm: VmTracer::init(),
            data: vec![],
            enable_return_data,
        }
    }

    #[must_use]
    pub fn into_traces(self) -> (Option<VMTrace>, Vec<FullTraceData>) {
        let vm = self.vm.tracer.drain();
        (vm, self.data)
    }
}
