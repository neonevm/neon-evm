use crate::{types::trace::{FullTraceData, VMTrace, VMTracer},};
use super::{vm_tracer::VmTracer,};


pub struct Tracer {
    pub vm: VmTracer,
    pub data: Vec<FullTraceData>,
}

impl Tracer  {
    pub fn new() -> Self {
        Tracer {
            vm: VmTracer::init(),
            data: vec![],
        }
    }

    pub fn into_traces(
        self,
    ) -> (
        Option<VMTrace>,
        Vec<FullTraceData>,
    ) {
        let vm = self.vm.tracer.drain();
        (vm, self.data)
    }
}

