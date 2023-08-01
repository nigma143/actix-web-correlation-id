use awc::ClientRequest;

use crate::{CorrelationIdHeader, CorrelationIdHeaderPropagate};

impl CorrelationIdHeaderPropagate for ClientRequest {
    fn with_correlation_id_header<T>(self, correlation_id_header: T) -> Self
    where
        T: Into<CorrelationIdHeader>,
    {
        self.insert_header(correlation_id_header.into())
    }
}

#[cfg(test)]
mod test {
    use actix_web::http::header::{HeaderName, HeaderValue};

    use crate::{CorrelationIdGenerator, CorrelationIdHeaderPropagate, UuidCorrelationIdGenerator};

    #[test]
    fn test_client_request_with_correlation_id_header() {
        let header_name_str = "x-request-id";
        let client = awc::Client::default();
        let correlation_id = UuidCorrelationIdGenerator
            .generate_correlation_id()
            .unwrap();
        let request = client
            .get("http://www.rust-lang.org")
            .with_correlation_id_header((
                HeaderName::from_static(header_name_str),
                correlation_id.clone(),
            ));

        let header_value = request.headers().get(header_name_str);

        assert_eq!(
            correlation_id.parse::<HeaderValue>().ok(),
            header_value.cloned()
        );
    }
}
