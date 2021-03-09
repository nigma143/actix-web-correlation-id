use std::{io::Read, sync::Arc};

use actix_web::{
    dev::ServiceRequest,
    error::{ErrorForbidden, ErrorInternalServerError},
    web::{self},
    App, Error, HttpServer, Responder,
};
use futures::future::{ready, Ready};

async fn index() -> impl Responder {
    "this_is_response_body"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new().service(web::resource("/simple").route(web::post().to(index)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
