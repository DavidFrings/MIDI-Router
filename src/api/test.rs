use actix_web::{HttpResponse, Responder, get};
use log::info;

#[get("/test")]
pub(crate) async fn test() -> impl Responder {
    info!("Hello endpoint called");
    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"message": "Hello from MIDI Router API!"}"#)
}
