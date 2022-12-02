use crate::{
    types::ec::trace::{
        ExecutiveTracer, FlatTrace,
        FullTraceData, Tracer as _, VMTrace, VMTracer
    },
};
use super::{
    vm_tracer::VmTracer,
};


pub struct Tracer {
    pub vm: VmTracer,
    pub tracer: ExecutiveTracer,
    pub data: Vec<FullTraceData>,
    pub return_value: Vec<u8>,
}


impl Tracer  {
    pub fn new() -> Self {
        Tracer {
            vm: VmTracer::init(),
            tracer: ExecutiveTracer::default(),
            data: vec![],
            return_value: vec![],
        }
    }

    pub fn into_traces(
        self,
    ) -> (
        Option<VMTrace>,
        Vec<FlatTrace>,
        Vec<FullTraceData>,
        Vec<u8>,
    ) {
        let vm = self.vm.tracer.drain();
        let traces = self.tracer.drain();
        (vm, traces, self.data, self.return_value)
    }
}

