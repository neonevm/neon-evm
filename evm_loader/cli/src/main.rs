#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]

mod account_storage;
mod commands;
pub mod config;
mod errors;
mod event_listener;
mod logs;
mod program_options;
mod rpc;
mod syscall_stubs;
mod types;

use clap::ArgMatches;
pub use config::Config;

use crate::errors::NeonCliError;

type NeonCliResult = Result<serde_json::Value, NeonCliError>;

fn run(options: &ArgMatches) -> NeonCliResult {
    let (cmd, params) = options.subcommand();
    let config = config::create(options)?;

    commands::execute(cmd, params, &config)
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
    let options = program_options::parse();

    logs::init(&options).expect("logs init error");
    std::panic::set_hook(Box::new(|info| {
        let message = std::format!("Panic: {}", info);
        print_result(&Err(NeonCliError::Panic(message)));
    }));

    let result = run(&options);

    print_result(&result);
    if let Err(e) = result {
        std::process::exit(e.error_code());
    };
}
