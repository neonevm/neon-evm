use jsonrpc_v2::{Data, Server};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let matches = clap::App::new("Neon Core API")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Neon Labs")
        .about("Runs a Neon Core API server")
        .arg(
            clap::Arg::with_name("LIB-DIR")
                .help("Directory with neon libraries to load")
                .required(true)
                .index(1),
        )
        .get_matches();

    let lib_dir = matches.value_of("LIB-DIR").unwrap();
    println!("LIB-DIR loaded: {lib_dir}");

    let libraries = neon_interface::load_libraries(lib_dir)?;

    let rpc = Server::new().with_data(Data::new(libraries)).finish();

    actix_web::HttpServer::new(move || {
        let rpc = rpc.clone();
        actix_web::App::new().service(
            actix_web::web::service("/")
                .guard(actix_web::guard::Post())
                .finish(rpc.into_web_service()),
        )
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await?;

    Ok(())
}
