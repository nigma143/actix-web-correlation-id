use actix_web::client::ClientRequest;

use crate::middleware::{CorrelationId, CorrelationIdPropagate};

impl CorrelationIdPropagate for ClientRequest {
    fn with_corr_id(self, v: CorrelationId) -> Self {
        self.set_header(v.get_key(), v.get_value())
    }
}
