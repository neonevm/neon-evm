//! # ERC20 Token Faucet (Airdrop)
//! ERC20 Token Faucet is a service which performs airdrop of tokens on user request.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod airdrop;
mod cli;
mod config;
mod server;
mod tokens;

use color_eyre::Result;
use tracing::info;
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
    let ver = env!("VERGEN_GIT_SEMVER");
    let rev = env!("VERGEN_GIT_SHA");
    let rev = if rev.len() < 7 {
        rev.to_string()
    } else {
        rev[..7].to_string()
    };
    info!("version {} (revision {})", ver, rev);
}

/// Dispatches CLI commands.
async fn execute(app: cli::Application) -> Result<()> {
    info_version();

    match app.cmd {
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
    config::load(config_file)?;
    tokens::init(config::tokens())?;
    server::start(config::rpc_port(), workers).await?;
    Ok(())
}
