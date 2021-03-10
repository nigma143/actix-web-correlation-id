[![Documentation](https://docs.rs/actix-web-correlation-id/badge.svg)](https://docs.rs/actix-web-correlation-id)
[![crates.io](https://img.shields.io/crates/v/actix-web-correlation-id.svg)](https://crates.io/crates/actix-web-correlation-id)

# actix-web-correlation-id

An Actix-web middleware component which synchronises a correlation ID for cross API request logging

## Example:
```rust
use actix_web::{
    client::Client,
    middleware::Logger,
    web::{self},
    App, Error, HttpResponse, HttpServer,
};
use actix_web_correlation_id::{
    Correlation, CorrelationId, CorrelationIdPropagate, CorrelationIdVariable,
};

async fn index(corr_id: CorrelationId) -> Result<HttpResponse, Error> {
    let client = Client::new();

    let mut res = client
        .get("http://google.com/")
        .with_corr_id(corr_id)
        .send()
        .await?;

    let mut client_resp = HttpResponse::build(res.status());

    Ok(client_resp.body(res.body().await?))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(
                Logger::new("%{corr-id}xi %a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T")
                    .add_corr_id(),
            )
            .wrap(
                Correlation::new()
                    .header_name("x-correlation-id")
                    .enforce_header(false)
                    .resp_header_name(Some("x-correlation-id"))
                    .include_in_resp(true),
            )
            .service(web::resource("/simple").route(web::post().to(index)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```
