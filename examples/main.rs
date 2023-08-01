use std::fmt;

use actix_web::{
    error,
    http::{
        header::{ContentType, HeaderName},
        StatusCode,
    },
    middleware::Logger,
    web::{self},
    App, HttpMessage, HttpResponse, HttpServer,
};
use actix_web_correlation_id::{
    Correlation, CorrelationId, CorrelationIdHeaderPropagate, CorrelationIdVariable,
};
use awc::Client;

#[derive(Debug)]
enum AppError {
    SendRequestError(String),
    ResponseBodyError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::SendRequestError(msg) => write!(f, "failed to send request: {}", msg),
            AppError::ResponseBodyError(msg) => write!(f, "failed to get response body: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .content_type(ContentType::plaintext())
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            AppError::SendRequestError(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::ResponseBodyError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

async fn index(corr_id: CorrelationId) -> Result<HttpResponse, AppError> {
    let client = Client::new();

    let mut res = client
        .get("http://www.rust-lang.org/")
        .with_correlation_id_header((HeaderName::from_static("x-correlation-id"), corr_id))
        .send()
        .await
        .map_err(|send_request_error| AppError::SendRequestError(send_request_error.to_string()))?;

    res.body()
        .await
        .map_err(|payload_error| AppError::ResponseBodyError(payload_error.to_string()))
        .map(|body| {
            HttpResponse::build(res.status())
                .content_type(res.content_type())
                .body(body)
        })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(
                Logger::new("%{corr-id}xi %a \"%r\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T")
                    .add_correlation_id(),
            )
            .wrap(
                Correlation::default()
                    .request_header_name(HeaderName::from_static("x-request-id"))
                    .enforce_request_header(false)
                    .response_header_name(HeaderName::from_static("x-correlation-id"))
                    .include_in_response(true),
            )
            .service(web::resource("/simple").route(web::post().to(index)))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
