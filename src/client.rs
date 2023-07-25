use awc::ClientRequest;

use crate::middleware::{CorrelationId, CorrelationIdPropagate};

impl CorrelationIdPropagate for ClientRequest {
    fn with_corr_id(self, correlation_id: CorrelationId) -> Self {
        self.insert_header((correlation_id.get_key(), correlation_id.get_value()))
    }
}
