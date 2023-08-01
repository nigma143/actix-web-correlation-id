use actix_web::http::header::HeaderName;

use crate::{CorrelationIdGenerator, UuidCorrelationIdGenerator};

const DEFAULT_HEADER_NAME: &str = "x-correlation-id";

pub(crate) struct Config {
    pub(crate) header_name: HeaderName,
    pub(crate) enforce_header: bool,
    pub(crate) resp_header_name: HeaderName,
    pub(crate) include_in_resp: bool,
    pub(crate) correlation_id_generator: Box<dyn CorrelationIdGenerator>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            header_name: HeaderName::from_static(DEFAULT_HEADER_NAME),
            enforce_header: false,
            resp_header_name: HeaderName::from_static(DEFAULT_HEADER_NAME),
            include_in_resp: true,
            correlation_id_generator: Box::new(UuidCorrelationIdGenerator),
        }
    }
}
