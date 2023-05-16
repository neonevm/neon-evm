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

use std::env;

use api_server::routes::register;
pub use config::Config;
pub use context::Context;
use errors::NeonCliError;
use log::LevelFilter;
use tide::{utils::After, Response, StatusCode};

type NeonCliResult = Result<serde_json::Value, NeonCliError>;

#[tokio::main]
async fn main() -> tide::Result<()> {
    let options = api_options::parse();

    let api_config = config::load_api_config_from_enviroment();

    let config = config::create_from_api_comnfig(&api_config)?;

    let state = api_server::state::State::new(config);

    let mut app = tide::with_state(state.clone());

    femme::with_level(LevelFilter::Debug);

    app.with(After(|mut res: Response| async {
        let err = res.take_error();
        if let Some(err) = err {
            let err_string = err.to_string();
            let value = serde_json::from_str(err_string.as_str())?;
            if let serde_json::Value::Object(map) = value {
                let err_result = serde_json::json!({
                    "result": "error",
                    "value": map.get("error"),
                });
                res.set_status(StatusCode::BadRequest);
                res.set_body(serde_json::to_string_pretty(&err_result).unwrap());
            } else {
                let err_result = serde_json::json!({
                    "result": "error",
                    "value": &err_string,
                });
                res.set_status(StatusCode::BadRequest);
                res.set_body(serde_json::to_string_pretty(&err_result).unwrap());
            }
        };
        Ok(res)
    }));

    app.at("/api").nest(register(state));

    let listener_addr = options
        .value_of("host")
        .map(std::borrow::ToOwned::to_owned)
        .map_or_else(
            || "0.0.0.0:8080".to_owned(),
            |_| env::var("NEON_API_LISTENER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
        );

    println!("API Server will be started on {}", listener_addr);

    app.listen(listener_addr).await?;

    Ok(())
}
