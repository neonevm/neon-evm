#![allow(dead_code)]
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
mod account_storage;
mod api_options;
mod api_server;
mod commands;
mod config;
mod context;
mod errors;
mod event_listener;
mod program_options;
mod rpc;
mod syscall_stubs;
mod types;

use std::{env, net::SocketAddr, str::FromStr, sync::Arc};

use axum::Router;
pub use config::Config;
pub use context::Context;
use errors::NeonCliError;
use tokio::signal::{self};

type NeonCliResult = Result<serde_json::Value, NeonCliError>;
type NeonApiResult<T> = Result<T, NeonCliError>;
type NeonApiState = Arc<api_server::state::State>;

#[tokio::main]
async fn main() -> NeonApiResult<()> {
    let options = api_options::parse();

    // initialize tracing
    tracing_subscriber::fmt::init();

    let api_config = config::load_api_config_from_enviroment();

    let config = config::create_from_api_comnfig(&api_config)?;

    let state: NeonApiState = Arc::new(api_server::state::State::new(config)) as NeonApiState;

    let app = Router::new()
        .nest("/api", api_server::routes::register(state.clone()))
        .with_state(state.clone());

    let listener_addr = options
        .value_of("host")
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || "0.0.0.0:8080".to_owned(),
            |_| env::var("NEON_API_LISTENER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        );

    let addr = SocketAddr::from_str(listener_addr.as_str())?;
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
}
