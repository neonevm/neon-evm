//! Faucet server implementation.

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::web::{post, Bytes};
use actix_web::{App, HttpResponse, HttpServer, Responder};
use color_eyre::Result;
use tracing::{error, info};

use crate::{config, eth_token, tokens};

/// Starts the server in listening mode.
pub async fn start(rpc_port: u16, workers: usize) -> Result<()> {
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
            .route("/request_eth_token", post().to(handle_request_eth_token))
            .route(
                "/request_erc20_tokens",
                post().to(handle_request_erc20_tokens),
            )
    })
    .bind(("localhost", rpc_port))?
    .workers(workers)
    .run()
    .await?;

    Ok(())
}

/// Handles a request for ETH token airdrop.
async fn handle_request_eth_token(body: Bytes) -> impl Responder {
    println!();
    info!("Handling Request for ETH Airdrop...");

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    let airdrop = serde_json::from_str::<eth_token::Airdrop>(&input.unwrap());
    if let Err(err) = airdrop {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = eth_token::airdrop(airdrop.unwrap()).await {
        error!("InternalServerError: {}", err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}

/// Handles a request for ERC20 tokens airdrop.
async fn handle_request_erc20_tokens(body: Bytes) -> impl Responder {
    println!();
    info!("Handling Request for ERC20 Airdrop...");

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    let airdrop = serde_json::from_str::<tokens::Airdrop>(&input.unwrap());
    if let Err(err) = airdrop {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = tokens::airdrop(airdrop.unwrap()).await {
        error!("InternalServerError: {}", err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}
