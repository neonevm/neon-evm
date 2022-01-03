//! Faucet server implementation.

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{post, Bytes};
use actix_web::{App, HttpResponse, HttpServer, Responder};
use color_eyre::Result;
use tracing::{error, info};

use crate::{config, eth_token, tokens};

/// Starts the server in listening mode.
pub async fn start(rpc_bind: &str, rpc_port: u16, workers: usize) -> Result<()> {
    info!("Bind {}:{}", rpc_bind, rpc_port);

    HttpServer::new(|| {
        let mut cors = Cors::default()
            .allowed_methods(vec!["POST"])
            .allowed_header(header::CONTENT_TYPE)
            .max_age(3600);
        for origin in &config::allowed_origins() {
            cors = cors.allowed_origin(origin);
        }
        App::new()
            .wrap(cors)
            .route(
                "/request_neon_in_galans",
                post().to(handle_request_neon_in_galans),
            )
            .route("/request_neon", post().to(handle_request_neon))
            .route("/request_erc20", post().to(handle_request_erc20))
            .route("/request_stop", post().to(handle_request_stop))
    })
    .bind((rpc_bind, rpc_port))?
    .workers(workers)
    .run()
    .await?;

    Ok(())
}

/// Handles a request for NEON airdrop in galans (1 galan = 10E-9 NEON).
async fn handle_request_neon_in_galans(body: Bytes) -> impl Responder {
    let id = generate_id();

    println!();
    info!("{} Handling Request for NEON (in galans) Airdrop...", id);

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("{} BadRequest (body): {}", id, err);
        return HttpResponse::BadRequest();
    }

    let input = input.unwrap();
    let airdrop = serde_json::from_str::<eth_token::Airdrop>(&input);
    if let Err(err) = airdrop {
        error!("{} BadRequest (json): {} in '{}'", id, err, input);
        return HttpResponse::BadRequest();
    }

    let mut airdrop = airdrop.unwrap();
    airdrop.in_fractions = true;
    if let Err(err) = eth_token::airdrop(id.clone(), airdrop).await {
        error!("{} InternalServerError: {}", id, err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}

/// Handles a request for NEON airdrop.
async fn handle_request_neon(body: Bytes) -> impl Responder {
    let id = generate_id();

    println!();
    info!("{} Handling Request for NEON Airdrop...", id);

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("{} BadRequest (body): {}", id, err);
        return HttpResponse::BadRequest();
    }

    let input = input.unwrap();
    let airdrop = serde_json::from_str::<eth_token::Airdrop>(&input);
    if let Err(err) = airdrop {
        error!("{} BadRequest (json): {} in '{}'", id, err, input);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = eth_token::airdrop(id.clone(), airdrop.unwrap()).await {
        error!("{} InternalServerError: {}", id, err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}

/// Handles a request for ERC20 tokens airdrop.
async fn handle_request_erc20(body: Bytes) -> impl Responder {
    let id = generate_id();

    println!();
    info!("{} Handling Request for ERC20 Airdrop...", id);

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("{} BadRequest (body): {}", id, err);
        return HttpResponse::BadRequest();
    }

    let input = input.unwrap();
    let airdrop = serde_json::from_str::<tokens::Airdrop>(&input);
    if let Err(err) = airdrop {
        error!("{} BadRequest (json): {} in '{}'", id, err, input);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = tokens::airdrop(id.clone(), airdrop.unwrap()).await {
        error!("{} InternalServerError: {}", id, err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}

/// Represents packet of information needed for the stop.
#[derive(Debug, serde::Deserialize)]
pub struct Stop {
    /// Milliseconds to wait before shutdown.
    delay: u64,
}

/// Handles a request for graceful shutdown.
async fn handle_request_stop(body: Bytes) -> impl Responder {
    use nix::sys::signal;
    use nix::unistd::Pid;
    use tokio::time::Duration;

    info!("Shutting down...");

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("BadRequest (body): {}", err);
        return HttpResponse::BadRequest();
    }

    let input = input.unwrap();
    let stop = serde_json::from_str::<Stop>(&input);
    if let Err(err) = stop {
        error!("BadRequest (json): {} in '{}'", err, input);
        return HttpResponse::BadRequest();
    }

    let delay = stop.unwrap().delay;
    if delay > 0 {
        info!("Sleeping {} millis...", delay);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    let terminate = signal::kill(Pid::this(), signal::SIGTERM);
    if let Err(err) = terminate {
        error!("BadRequest (terminate): {}", err);
        return HttpResponse::BadRequest();
    }

    HttpResponse::Ok()
}

/// Builds a (hopefully) unique string to mark requests.
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards");
    let digest = md5::compute(since.as_millis().to_string());
    let s = format!("{:x}", digest)[..7].to_string();
    format!("[{}]", s)
}
