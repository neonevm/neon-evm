//! # NeonLabs Faucet Service
//! NeonLabs Faucet is a service which provides tokens to users.

#![deny(warnings)]

mod cli;
mod config;
mod erc20_tokens;
mod ethereum;
mod manual;
mod neon_token;
mod server;
mod solana;

use color_eyre::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<()> {
    setup()?;
    show_version();
    execute(cli::application()).await
}

/// Initializes the logger and error handler.
fn setup() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "0")
    }
    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}

macro_rules! faucet_pkg_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}
macro_rules! faucet_revision {
    () => {
        env!("NEON_REVISION")
    };
}
macro_rules! version_string {
    () => {
        concat!("Faucet/v", faucet_pkg_version!(), "-", faucet_revision!())
    };
}

/// Shows semantic version and revision hash.
fn show_version() {
    info!(version_string!());
}

/// Dispatches CLI commands.
async fn execute(app: cli::Application) -> Result<()> {
    match app.cmd {
        cli::Command::Config { file } => {
            config::check_file_exists(&file);
            config::load(&file)?;
            config::show();
        }
        cli::Command::Env {} => {
            config::show_env();
        }
        cli::Command::Man { api, config, env } => {
            manual::show(api, config, env);
        }
        cli::Command::Run { workers } => {
            let workers = if workers == config::AUTO {
                num_cpus::get()
            } else {
                workers.parse::<usize>()?
            };
            run(&app.config, workers).await?;
            info!("Done.");
        }
    }

    Ok(())
}

use std::path::Path;

/// Runs the server.
async fn run(config_file: &Path, workers: usize) -> Result<()> {
    config::check_file_exists(config_file);
    config::load(config_file)?;
    config::show();

    if config::solana_enabled() {
        solana::init_client(config::solana_url());
    }

    if config::web3_enabled() || config::solana_enabled() {
        server::start(&config::rpc_bind(), config::rpc_port(), workers).await?;
    }

    Ok(())
}
