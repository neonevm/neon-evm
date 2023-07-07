#![allow(dead_code)]
#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
mod api_options;
mod api_server;

use actix_web::web;
use actix_web::App;
use actix_web::HttpServer;
use api_server::handlers::NeonApiError;
pub use neon_lib::account_storage;
pub use neon_lib::commands;
pub use neon_lib::config;
pub use neon_lib::context;
pub use neon_lib::errors;
pub use neon_lib::event_listener;
pub use neon_lib::rpc;
pub use neon_lib::syscall_stubs;
pub use neon_lib::types;

use std::sync::Arc;
use std::{env, net::SocketAddr, str::FromStr};

pub use config::Config;
pub use context::Context;
use tokio::signal::{self};

use crate::api_server::handlers::{
    emulate::emulate, emulate_hash::emulate_hash, get_ether_account_data::get_ether_account_data,
    get_storage_at::get_storage_at, trace::trace, trace_hash::trace_hash,
};

type NeonApiResult<T> = Result<T, NeonApiError>;
type NeonApiState = Arc<api_server::state::State>;

#[tokio::main]
async fn main() -> NeonApiResult<()> {
    let options = api_options::parse();

    // initialize tracing
    tracing_subscriber::fmt::init();

    let api_config = config::load_api_config_from_enviroment();

    let config = config::create_from_api_comnfig(&api_config)?;

    let state: NeonApiState = Arc::new(api_server::state::State::new(config));

    let listener_addr = options
        .value_of("host")
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || "0.0.0.0:8080".to_owned(),
            |_| env::var("NEON_API_LISTENER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        );

    let addr = SocketAddr::from_str(listener_addr.as_str())?;
    tracing::debug!("listening on {}", addr);
    HttpServer::new(move || {
        App::new().service(
            web::scope("/api")
                .app_data(state.clone())
                .service(emulate)
                .service(emulate_hash)
                .service(get_ether_account_data)
                .service(get_storage_at)
                .service(trace)
                .service(trace_hash),
        )
    })
    .bind(&addr)
    .unwrap()
    .run()
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
