#![deny(warnings)] //TODO
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::cast_possible_wrap)]

mod account_storage;
mod syscall_stubs;
mod errors;
mod logs;
mod commands;
mod rpc;
mod program_options;
pub mod config;
mod event_listener;
mod types;

pub use config::Config;

use std::process::exit;
use log::error;
use crate::errors::NeonCliError;

type NeonCliResult = Result<(),NeonCliError>;

#[tokio::main]
async fn main() {
    let options = program_options::parse();

    logs::init(&options).unwrap();

    let config = config::create(&options);

    let (cmd, params) = options.subcommand();

    match commands::execute(cmd, params, &config) {
        Ok(_)  => exit(0),
        Err(e) => {
            let code = e.error_code();
            error!("NeonCli Error ({}): {}", code, e);
            exit(code as i32)
        }
    }
}
