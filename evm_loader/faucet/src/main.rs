//! # ERC20 Token Faucet (Airdrop)
//! ERC20 Token Faucet is a service which performs airdrop of tokens on user request.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod airdrop;
mod cli;
mod config;
mod server;

use color_eyre::Report;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<(), Report> {
    setup()?;
    execute(cli::application()).await
}

/// Initializes the logger and error handler.
fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
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

/// Dispatches CLI commands.
async fn execute(app: cli::Application) -> Result<(), Report> {
    match app.cmd {
        cli::Command::Run => {
            run(&app.config).await?;
        }
    }

    info!("Done.");
    Ok(())
}

/// Runs the server.
async fn run(config_file: &std::path::Path) -> Result<(), Report> {
    config::load(config_file)?;
    server::start(config::rpc_port()).await?;
    Ok(())
}
