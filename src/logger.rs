use actix_web::middleware::Logger;

use crate::{CorrelationIdExtract, CorrelationIdVariable};

impl CorrelationIdVariable for Logger {
    fn add_correlation_id(self) -> Self {
        self.custom_request_replace("corr-id", |req| req.correlation_id().to_string())
    }
}
