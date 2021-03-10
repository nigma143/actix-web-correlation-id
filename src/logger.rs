use actix_web::middleware::Logger;

use crate::middleware::{CorrelationIdExtract, CorrelationIdVariable};

impl CorrelationIdVariable for Logger {
    fn add_corr_id(self) -> Self {
        self.custom_request_replace("corr-id", |req| req.corr_id().get_value().into())
    }
}
