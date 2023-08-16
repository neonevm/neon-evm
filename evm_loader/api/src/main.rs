#![allow(dead_code)]
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
mod api_options;
mod api_server;

use api_server::handlers::NeonApiError;
use axum::Router;
pub use neon_lib::account_storage;
pub use neon_lib::commands;
pub use neon_lib::config;
pub use neon_lib::context;
pub use neon_lib::errors;
pub use neon_lib::rpc;
pub use neon_lib::syscall_stubs;
pub use neon_lib::types;
use tracing_appender::non_blocking::NonBlockingBuilder;

use std::sync::Arc;
use std::{env, net::SocketAddr, str::FromStr};

pub use config::Config;
pub use context::Context;
use tokio::signal::{self};

type NeonApiResult<T> = Result<T, NeonApiError>;
type NeonApiState = Arc<api_server::state::State>;

#[tokio::main(flavor = "multi_thread", worker_threads = 512)]
async fn main() -> NeonApiResult<()> {
    let options = api_options::parse();

    // initialize tracing
    let (non_blocking, _guard) = NonBlockingBuilder::default()
        .lossy(false)
        .finish(std::io::stdout());

    tracing_subscriber::fmt().with_writer(non_blocking).init();

    let api_config = config::load_api_config_from_enviroment();

    let config = config::create_from_api_config(&api_config)?;

    let state: NeonApiState = Arc::new(api_server::state::State::new(config));

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
