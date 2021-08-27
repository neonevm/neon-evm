//! # ERC20 Token Faucet (Airdrop)
//! ERC20 Token Faucet is a service which performs airdrop of tokens on user request.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod airdrop;
mod cli;
mod config;
mod ethereum;
mod server;
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

/// Runs the server.
async fn run(config_file: &std::path::Path, workers: usize) -> Result<()> {
    check_file_exists(config_file);
    config::load(config_file)?;
    config::show();

    tokens::init(config::tokens()).await?;
    server::start(config::rpc_port(), workers).await?;

    Ok(())
}

fn check_file_exists(file: &std::path::Path) {
    if !file.exists() {
        warn!(
            "File {:?} is missing; will use settings from environment variables (if any)",
            file
        );
    }
}
