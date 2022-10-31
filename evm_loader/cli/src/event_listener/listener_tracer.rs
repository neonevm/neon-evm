use super::{
    tracer::Tracer,
};
use crate::{
    types::ec::trace::{FullTraceData},
};

pub trait ListenerTracer {
    fn begin_step(&mut self, stack: Vec<[u8; 32]>, memory: Vec<u8>);
    fn end_step(&mut self);
}

impl ListenerTracer for Tracer {
    fn begin_step(&mut self, stack: Vec<[u8; 32]>, memory: Vec<u8>) {
        let storage = self.data.last()
            .map(|d| d.storage.clone())
            .unwrap_or_default();

        self.data.push(FullTraceData { stack, memory, storage });
    }

    fn end_step(&mut self) {
        if let Some((index, value)) = self.vm.step_diff().storage_access {
            let data = self.data.last_mut().expect("data was pushed in begin_step");
            data.storage.insert(index, value);
        }
    }
}
