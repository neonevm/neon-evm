use evm_loader::StepTrace;
use super::{
    tracer::Tracer,
};
use crate::{
    types::ec::trace::{FullTraceData},
};

pub trait ListenerTracer {
    fn step (&mut self, mes: &StepTrace);
}

impl ListenerTracer for Tracer{

    fn step (&mut self, mes: &StepTrace){
        if let Some((index, value)) = self.vm.storage_accessed.take() {
            if let Some(data) = self.data.last_mut() {
                data.storage = Some((index, value));
            }
        }

        let stack = (0..mes.stack.len())
            .rev()
            .map(|i| mes.stack.peek(i).unwrap())
            .collect::<Vec<_>>();
        let memory = mes.memory.data().to_vec();
        self.data.push(FullTraceData {
            stack: stack.clone(),
            memory: memory.clone(),
            storage: None,
        });
    }
}
