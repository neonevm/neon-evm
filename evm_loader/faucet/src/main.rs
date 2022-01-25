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
mod version;

use eyre::Result;
use tracing::info;

#[actix_web::main]
async fn main() -> Result<()> {
    setup()?;
    show_version();
    execute(cli::application()).await
}

/// Initializes the logger.
fn setup() -> Result<()> {
    use std::env;
    use time::macros::format_description;
    use time::UtcOffset;
    use tracing_subscriber::fmt::time::OffsetTime;
    use tracing_subscriber::EnvFilter;

    if env::var("RUST_LIB_BACKTRACE").is_err() {
        env::set_var("RUST_LIB_BACKTRACE", "0")
    }

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }

    if env::var("NEON_LOG").is_err() {
        env::set_var("NEON_LOG", "plain")
    }
    let json = env::var("NEON_LOG").unwrap().contains("json");

    let offset = UtcOffset::current_local_offset()?;
    let timer = OffsetTime::new(
        offset,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );

    if !json {
        tracing_subscriber::fmt::fmt()
            .with_timer(timer)
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    } else {
        tracing_subscriber::fmt::fmt()
            .with_timer(timer)
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .flatten_event(true)
            .init();
    }

    Ok(())
}

/// Shows semantic version and revision hash.
fn show_version() {
    info!(version::display!());
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
