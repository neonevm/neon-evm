#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_possible_wrap)]

mod account_storage;
mod syscall_stubs;
mod errors;
mod logs;
pub mod commands;
mod rpc;
mod program_options;
pub mod config;
mod event_listener;
mod types;

pub use config::Config;

use std::process::exit;
use crate::errors::NeonCliError;

type NeonCliResult = Result<serde_json::Value, NeonCliError>;

#[tokio::main]
async fn main() {
    let options = program_options::parse();

    logs::init(&options).expect("logs init error");

    let config = config::create(&options);

    let (cmd, params) = options.subcommand();

    let result = commands::execute(cmd, params, &config);
    let logs = {
        let context = crate::logs::CONTEXT.lock().unwrap();
        context.clone()
    };

    let (result, exit_code) = match result {
        Ok(result) => {
            (serde_json::json!({
                "result": "success",
                "value": result,
                "logs": logs
            }), 0_i32)
        }
        Err(e) => {
            let error_code = e.error_code() as i32;
            (serde_json::json!({
                "result": "error",
                "error": e.to_string(),
                "logs": logs
            }), error_code)
        }
    };

    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    exit(exit_code);
}
