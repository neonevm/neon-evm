use super::tracer::Tracer;
use crate::evm::tracing::event_listener::trace::FullTraceData;

pub trait ListenerTracer {
    fn begin_step(&mut self, stack: Vec<[u8; 32]>, memory: Vec<u8>);
    fn end_step(&mut self, return_data: Option<Vec<u8>>);
}

impl ListenerTracer for Tracer {
    fn begin_step(&mut self, stack: Vec<[u8; 32]>, memory: Vec<u8>) {
        let storage = self
            .data
            .last()
            .map(|d| d.storage.clone())
            .unwrap_or_default();

        self.data.push(FullTraceData {
            stack,
            memory,
            storage,
            return_data: None,
        });
    }

    fn end_step(&mut self, return_data: Option<Vec<u8>>) {
        let data = self
            .data
            .last_mut()
            .expect("No data were pushed in `begin_step`");
        data.return_data = return_data;
        if let Some((index, value)) = self.vm.step_diff().storage_access {
            data.storage.insert(index, value);
        }
    }
}
