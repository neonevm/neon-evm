#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]

mod commands;
mod config;
mod context;
mod logs;
mod program_options;

pub use neon_lib::account_storage;
pub use neon_lib::errors;
pub use neon_lib::event_listener;
pub use neon_lib::rpc;
pub use neon_lib::syscall_stubs;
pub use neon_lib::types;

use clap::ArgMatches;
pub use config::Config;
pub use context::Context;

use crate::errors::NeonError;
use std::sync::Arc;
use std::time::Instant;

type NeonCliResult = Result<serde_json::Value, NeonError>;

async fn run<'a>(options: &'a ArgMatches<'a>) -> NeonCliResult {
    let (cmd, params) = options.subcommand();
    let config = Arc::new(config::create(options)?);
    let context: Context = context::create_from_config_and_options(options, config.clone()).await?;

    commands::execute(cmd, params, config.as_ref(), &context).await
}

fn print_result(result: &NeonCliResult) {
    let logs = {
        let context = crate::logs::CONTEXT.lock().unwrap();
        context.clone()
    };

    let result = match result {
        Ok(value) => serde_json::json!({
            "result": "success",
            "value": value,
            "logs": logs
        }),
        Err(e) => serde_json::json!({
            "result": "error",
            "error": e.to_string(),
            "logs": logs
        }),
    };

    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

#[tokio::main]
async fn main() {
    let time_start = Instant::now();

    let options = program_options::parse();

    logs::init(&options).expect("logs init error");
    std::panic::set_hook(Box::new(|info| {
        let message = std::format!("Panic: {}", info);
        print_result(&Err(NeonError::Panic(message)));
    }));

    let result = run(&options).await;

    let execution_time = Instant::now().duration_since(time_start);
    log::info!("execution time: {} sec", execution_time.as_secs_f64());
    print_result(&result);
    if let Err(e) = result {
        std::process::exit(e.error_code());
    };
}
