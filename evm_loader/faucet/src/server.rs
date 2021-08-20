//! Faucet server implementation.

use actix_cors::Cors;
use actix_web::web::{post, Bytes};
use actix_web::{App, HttpResponse, HttpServer, Responder};
use color_eyre::Result;
use tracing::{error, info};

use crate::airdrop;

/// Starts the server in listening mode.
pub async fn start(rpc_port: u16, workers: usize) -> Result<()> {
    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_origin("http://localhost")
            .allowed_origin("http://neonlabs.org");
        App::new()
            .wrap(cors)
            .route("/request_airdrop", post().to(handle_request_airdrop))
    })
    .bind(("localhost", rpc_port))?
    .workers(workers)
    .run()
    .await?;
    Ok(())
}

/// Handles a request for airdrop.
async fn handle_request_airdrop(body: Bytes) -> impl Responder {
    println!();
    info!("Handling Request for Airdrop...");

    let input = String::from_utf8(body.to_vec());
    if let Err(err) = input {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    let airdrop = serde_json::from_str::<airdrop::Airdrop>(&input.unwrap());
    if let Err(err) = airdrop {
        error!("BadRequest: {}", err);
        return HttpResponse::BadRequest();
    }

    if let Err(err) = airdrop::process(airdrop.unwrap()).await {
        error!("InternalServerError: {}", err);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
}
