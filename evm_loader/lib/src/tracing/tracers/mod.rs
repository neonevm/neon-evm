use crate::tracing::tracers::struct_logger::StructLogger;
use crate::tracing::TraceConfig;
use evm_loader::evm::database::Database;
use evm_loader::evm::tracing::{Event, EventListener};
use serde_json::Value;

pub mod struct_logger;

pub enum TracerTypeEnum {
    StructLogger(StructLogger),
}

impl EventListener for TracerTypeEnum {
    fn event(&mut self, executor_state: &impl Database, event: Event) {
        match self {
            TracerTypeEnum::StructLogger(struct_logger) => {
                struct_logger.event(executor_state, event)
            }
        }
    }
}

pub trait Tracer: EventListener {
    fn into_traces(self) -> Value;
}

impl Tracer for TracerTypeEnum {
    fn into_traces(self) -> Value {
        match self {
            TracerTypeEnum::StructLogger(struct_logger) => struct_logger.into_traces(),
        }
    }
}

pub fn new_tracer(trace_config: &TraceConfig) -> evm_loader::error::Result<TracerTypeEnum> {
    match trace_config.tracer.as_deref() {
        None | Some("") => Ok(TracerTypeEnum::StructLogger(StructLogger::new(
            trace_config,
        ))),
        _ => Err(evm_loader::error::Error::Custom(format!(
            "Unsupported tracer: {:?}",
            trace_config.tracer
        ))),
    }
}
