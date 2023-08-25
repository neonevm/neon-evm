use crate::evm::tracing::tracers::struct_logger::StructLogger;
use crate::evm::tracing::TraceConfig;
use crate::evm::tracing::TracerType;
use std::sync::{Arc, RwLock};

pub mod struct_logger;

pub fn new_tracer(trace_config: &TraceConfig) -> crate::error::Result<TracerType> {
    Ok(Arc::new(RwLock::new(
        match trace_config.tracer.as_deref() {
            None | Some("") => Box::new(StructLogger::new(trace_config)),
            _ => {
                return Err(crate::error::Error::Custom(format!(
                    "Unsupported tracer: {:?}",
                    trace_config.tracer
                )))
            }
        },
    )))
}
