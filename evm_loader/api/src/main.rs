#![allow(dead_code)]
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
mod api_context;
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
use http::Request;
use hyper::Body;
use tokio::signal::{self};
use tower_http::trace::TraceLayer;
use tower_request_id::{RequestId, RequestIdLayer};
use tracing::info_span;

type NeonApiResult<T> = Result<T, NeonApiError>;
type NeonApiState = Arc<api_server::state::State>;

#[tokio::main(flavor = "multi_thread", worker_threads = 512)]
async fn main() -> NeonApiResult<()> {
    let options = api_options::parse();

    // initialize tracing
    let (non_blocking, _guard) = NonBlockingBuilder::default()
        .lossy(false)
        .finish(std::io::stdout());

    tracing_subscriber::fmt()
        .with_thread_ids(true)
        .with_writer(non_blocking)
        .init();

    let api_config = config::load_api_config_from_enviroment();

    let config = config::create_from_api_config(&api_config)?;

    let state: NeonApiState = Arc::new(api_server::state::State::new(config).await);

    let app = Router::new()
        .nest("/api", api_server::routes::register())
        .with_state(state)
        .layer(
            // Let's create a tracing span for each request
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                // We get the request id from the extensions
                let request_id = request
                    .extensions()
                    .get::<RequestId>()
                    .map_or_else(|| "unknown".into(), ToString::to_string);
                // And then we put it along with other information into the `request` span
                info_span!(
                    "request",
                    id = %request_id,
                )
            }),
        )
        // This layer creates a new id for each request and puts it into the request extensions.
        // Note that it should be added after the Trace layer.
        .layer(RequestIdLayer);

    let listener_addr = options
        .value_of("host")
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || "0.0.0.0:8080".to_owned(),
            |_| env::var("NEON_API_LISTENER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        );

    let addr = SocketAddr::from_str(listener_addr.as_str())?;
    tracing::info!("listening on {}", addr);
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
