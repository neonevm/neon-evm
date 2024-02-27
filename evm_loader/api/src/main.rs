#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic)]
mod api_options;
mod api_server;
#[allow(clippy::module_name_repetitions)]
mod build_info;

use actix_web::web;
use actix_web::App;
use actix_web::HttpServer;
use api_server::handlers::NeonApiError;
pub use neon_lib::commands;
pub use neon_lib::config;
pub use neon_lib::errors;
pub use neon_lib::types;
use tracing_appender::non_blocking::NonBlockingBuilder;

use actix_request_identifier::RequestIdentifier;
use actix_web::web::Data;
use std::{env, net::SocketAddr, str::FromStr};

use crate::api_server::handlers::build_info::build_info_route;
use crate::api_server::handlers::emulate::emulate;
use crate::api_server::handlers::get_balance::get_balance;
use crate::api_server::handlers::get_config::get_config;
use crate::api_server::handlers::get_contract::get_contract;
use crate::api_server::handlers::get_holder::get_holder_account_data;
use crate::api_server::handlers::get_storage_at::get_storage_at;
use crate::api_server::handlers::trace::trace;
use crate::build_info::get_build_info;
pub use config::Config;
use tracing::info;
use tracing_subscriber::EnvFilter;

type NeonApiResult<T> = Result<T, NeonApiError>;
type NeonApiState = Data<api_server::state::State>;

#[actix_web::main]
async fn main() -> NeonApiResult<()> {
    let options = api_options::parse();

    // initialize tracing
    let (non_blocking, _guard) = NonBlockingBuilder::default()
        .lossy(false)
        .finish(std::io::stdout());

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();

    info!("{}", get_build_info());

    let api_config = config::load_api_config_from_enviroment();
    let state: NeonApiState = Data::new(api_server::state::State::new(api_config));

    let listener_addr = options
        .value_of("host")
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || "0.0.0.0:8080".to_owned(),
            |_| env::var("NEON_API_LISTENER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        );

    let addr = SocketAddr::from_str(listener_addr.as_str())?;
    tracing::info!("listening on {}", addr);
    HttpServer::new(move || {
        App::new().service(
            web::scope("/api")
                .app_data(state.clone())
                .service(build_info_route)
                .service(emulate)
                .service(get_balance)
                .service(get_contract)
                .service(get_storage_at)
                .service(get_config)
                .service(get_holder_account_data)
                .service(trace)
                .wrap(RequestIdentifier::with_uuid()),
        )
    })
    .bind(addr)
    .unwrap()
    .run()
    .await
    .unwrap();

    Ok(())
}
