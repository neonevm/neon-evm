use crate::build_info::get_build_info;
use actix_web::get;
use actix_web::http::StatusCode;
use actix_web::web::Json;
use actix_web::Responder;

#[tracing::instrument(ret)]
#[get("/build-info")]
pub async fn build_info_route() -> impl Responder {
    (Json(get_build_info()), StatusCode::OK)
}
