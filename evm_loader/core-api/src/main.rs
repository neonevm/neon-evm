mod converter;

use converter::MethodConverters;
use jsonrpc_v2::{Data, Params, Server};
use neon_interface::NeonLib_Ref;
use semver::Version;
use std::{collections::HashMap, error::Error};

struct Context {
    libraries: HashMap<String, NeonLib_Ref>,
    method_converters: MethodConverters,
}

async fn get_hash() -> Result<String, String> {
    todo!()
}

async fn invoke(
    method: &str,
    context: Data<Context>,
    params: serde_json::Value,
) -> Result<serde_json::Value, jsonrpc_v2::Error> {
    let hash = get_hash().await.map_err(jsonrpc_v2::Error::internal)?;
    let library = context
        .libraries
        .get(&hash)
        .ok_or(jsonrpc_v2::Error::internal(format!(
            "Library not found for hash {hash}"
        )))?;

    let version = Version::parse(&library.get_version()()).map_err(jsonrpc_v2::Error::internal)?;

    let converter = context.method_converters.choose_converter(method, version);
    let method = converter.rename(method);
    let params = converter
        .convert_params(params)
        .map_err(jsonrpc_v2::Error::internal)?;

    let result: Result<_, _> = library.invoke()(
        method.as_str().into(),
        serde_json::to_string(&params).unwrap().as_str().into(),
    )
    .await
    .map(|x| serde_json::from_str::<serde_json::Value>(&x).unwrap())
    .map_err(String::from)
    .into();

    converter
        .convert_result(result)
        .map_err(jsonrpc_v2::Error::internal)
}

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

    let method_converters = MethodConverters::new();

    let context = Context {
        libraries,
        method_converters,
    };

    let methods = [
        "cancel_trx",
        "collect_treasury",
        "create_ether_account",
        "deposit",
        "emulate",
        "get_ether_account_data",
        "get_neon_elf",
        "get_storage_at",
        "init_environment",
    ];

    let mut rpc_builder = Server::new().with_data(Data::new(context));

    for method in methods {
        rpc_builder = rpc_builder.with_method(
            method,
            |context: Data<Context>, Params(params): Params<serde_json::Value>| {
                invoke(method, context, params)
            },
        );
    }

    let rpc = rpc_builder.finish();

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
