//! # ERC20 Token Faucet (Airdrop)
//! ERC20 Token Faucet is a service which performs airdrop of tokens on user request.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod cli;
mod config;
mod eth_token;
mod ethereum;
mod server;
mod solana;
mod tokens;

use color_eyre::Result;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<()> {
    setup()?;
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

/// Shows semantic version and revision hash.
fn info_version() {
    let ver = env!("CARGO_PKG_VERSION");
    let rev = if let Ok(rev) = std::env::var("VERGEN_GIT_SHA") {
        if rev.len() < 7 {
            rev
        } else {
            rev[..7].to_string()
        }
    } else {
        "<unknown>".to_owned()
    };
    info!("version {} (revision {})", ver, rev);
}

/// Dispatches CLI commands.
async fn execute(app: cli::Application) -> Result<()> {
    info_version();

    match app.cmd {
        cli::Command::Config { file } => {
            check_file_exists(&file);
            config::load(&file)?;
            config::show();
        }
        cli::Command::Env {} => {
            config::show_env();
        }
        cli::Command::Run { workers } => {
            let workers = if workers == config::AUTO {
                num_cpus::get()
            } else {
                workers.parse::<usize>()?
            };
            run(&app.config, workers).await?;
        }
    }

    info!("Done.");
    Ok(())
}

use std::path::Path;

/// Runs the server.
async fn run(config_file: &Path, workers: usize) -> Result<()> {
    check_file_exists(config_file);
    config::load(config_file)?;
    config::show();
    solana::init_client(config::solana_url());

    //ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ
    if false {
        tokens::init(config::tokens()).await?;
    }

    server::start(config::rpc_port(), workers).await
}

fn check_file_exists(file: &Path) {
    if !file.exists() {
        warn!("File {:?} is missing", file);
    }
}
