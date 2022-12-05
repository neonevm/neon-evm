use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::emulate};
use clap::ArgMatches;

pub fn execute(config: &Config, params: &ArgMatches) -> NeonCliResult {
    let mut tracer = Tracer::new();

    evm_loader::using( &mut tracer, || {
        emulate::execute(config, params)
    });

    NeonCliResult::Ok(())
}