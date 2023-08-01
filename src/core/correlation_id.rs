use actix_web::{
    dev::Payload,
    http::{
        header::{HeaderName, HeaderValue, InvalidHeaderValue, TryIntoHeaderPair},
        Error,
    },
    FromRequest, HttpMessage, HttpRequest,
};
use std::{
    fmt,
    future::{ready, Ready},
    ops::Deref,
    str::FromStr,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationId(String);

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for CorrelationId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for CorrelationId {
    type Err = CorrelationIdError;

    /// Attempt to convert a string to a `CorrelationId`.
    ///
    /// Only visible ASCII characters (32 - 127) are permitted as argument.
    /// At least one character is required.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.as_bytes())
    }
}

impl TryFrom<&[u8]> for CorrelationId {
    type Error = CorrelationIdError;

    /// Attempt to convert a byte array to a `CorrelationId`.
    ///
    /// Only visible ASCII characters (32 - 127) are permitted as argument.
    /// At least one character is required.
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(CorrelationIdError::Empty)
        } else {
            for (idx, b) in value.iter().copied().enumerate() {
                if !is_visible_ascii(b) {
                    return Err(CorrelationIdError::InvisibleAscii(idx));
                }
            }
            Ok(CorrelationId(String::from_utf8_lossy(value).to_string()))
        }
    }
}

const fn is_visible_ascii(b: u8) -> bool {
    32 <= b && 127 > b
}

impl TryFrom<String> for CorrelationId {
    type Error = CorrelationIdError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_bytes())
    }
}

impl FromRequest for CorrelationId {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        match req.extensions().get::<CorrelationId>() {
            Some(s) => ready(Ok(s.clone())),
            None => unreachable!("use correlation middleware in pipeline"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CorrelationIdError {
    Empty,
    InvisibleAscii(usize),
}

impl fmt::Display for CorrelationIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorrelationIdError::Empty => write!(f, "correlation ID is empty"),
            CorrelationIdError::InvisibleAscii(position_index) => {
                write!(f, "char at index {position_index} is non-visible ASCII")
            }
        }
    }
}

impl std::error::Error for CorrelationIdError {}

pub trait CorrelationIdGenerator {
    fn generate_correlation_id(&self) -> Result<CorrelationId, CorrelationIdError>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UuidCorrelationIdGenerator;

impl CorrelationIdGenerator for UuidCorrelationIdGenerator {
    fn generate_correlation_id(&self) -> Result<CorrelationId, CorrelationIdError> {
        CorrelationId::try_from(Uuid::new_v4().simple().to_string())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CorrelationIdHeader(pub(crate) HeaderName, pub(crate) CorrelationId);

impl From<(HeaderName, CorrelationId)> for CorrelationIdHeader {
    fn from(pair: (HeaderName, CorrelationId)) -> Self {
        CorrelationIdHeader(pair.0, pair.1)
    }
}

impl TryIntoHeaderPair for CorrelationIdHeader {
    type Error = InvalidHeaderValue;

    fn try_into_pair(self) -> Result<(HeaderName, HeaderValue), Self::Error> {
        self.1
            .parse::<HeaderValue>()
            .map(|header_value| (self.0, header_value))
    }
}

pub trait CorrelationIdVariable {
    fn add_correlation_id(self) -> Self;
}

pub trait CorrelationIdHeaderPropagate {
    fn with_correlation_id_header<T>(self, correlation_id_header: T) -> Self
    where
        T: Into<CorrelationIdHeader>;
}

pub trait CorrelationIdExtract {
    fn correlation_id(&self) -> CorrelationId;
}

impl<T> CorrelationIdExtract for T
where
    T: HttpMessage,
{
    fn correlation_id(&self) -> CorrelationId {
        if let Some(s) = self.extensions().get::<CorrelationId>() {
            s.clone()
        } else {
            unreachable!("use correlation middleware in pipeline");
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{
        http::header::{HeaderName, HeaderValue, TryIntoHeaderPair},
        test::TestRequest,
        HttpMessage,
    };
    use uuid::Uuid;

    use crate::{
        CorrelationId, CorrelationIdError, CorrelationIdExtract, CorrelationIdGenerator,
        CorrelationIdHeader, UuidCorrelationIdGenerator,
    };

    #[test]
    fn test_try_correlation_id_from_simple_uuid_v4() -> Result<(), CorrelationIdError> {
        let simple_uuid = Uuid::new_v4().simple().to_string();
        let correlation_id = CorrelationId::try_from(simple_uuid.clone())?;

        assert_eq!(correlation_id.to_string(), simple_uuid);

        Ok(())
    }

    #[test]
    fn try_parse_correlation_id_from_valid_str() {
        let parse_correlation_id_result = "a;lfjeaifaf".parse::<CorrelationId>();

        assert!(parse_correlation_id_result.is_ok());
    }

    #[test]
    fn try_parse_correlation_id_from_invalid_str() {
        let parse_correlation_id_result = "a;lfje…ifaf".parse::<CorrelationId>();

        assert_eq!(
            Err(CorrelationIdError::InvisibleAscii(6)),
            parse_correlation_id_result
        );
    }

    #[test]
    fn test_try_correlation_id_from_emtpy_string() {
        assert_eq!(
            Err(CorrelationIdError::Empty),
            CorrelationId::try_from("".to_string())
        );
    }

    #[test]
    fn test_try_correlation_id_from_string_with_invisible_ascii_char() {
        assert_eq!(
            Err(CorrelationIdError::InvisibleAscii(4)),
            CorrelationId::try_from("Hack€r".to_string())
        )
    }

    #[test]
    fn test_generate_correlation_id_with_uuid_generator() {
        let correlation_id_generator = UuidCorrelationIdGenerator;
        let generate_result = correlation_id_generator.generate_correlation_id();

        assert!(generate_result.is_ok());
    }

    #[test]
    fn test_correlation_id_header_from_pair() {
        let header_name = HeaderName::from_static("x-correlation-id");
        let correlation_id = UuidCorrelationIdGenerator
            .generate_correlation_id()
            .unwrap();

        assert_eq!(
            CorrelationIdHeader::from((header_name.clone(), correlation_id.clone())),
            CorrelationIdHeader(header_name, correlation_id)
        );
    }

    #[test]
    fn test_correlation_id_header_try_into_header_pair() {
        let header_name = HeaderName::from_static("x-request-id");
        let correlation_id = UuidCorrelationIdGenerator
            .generate_correlation_id()
            .unwrap();
        let correlation_id_header =
            CorrelationIdHeader::from((header_name.clone(), correlation_id.clone()));

        assert_eq!(
            correlation_id_header.try_into_pair().unwrap(),
            (header_name, HeaderValue::from_str(&correlation_id).unwrap())
        );
    }

    #[test]
    #[should_panic(expected = "use correlation middleware in pipeline")]
    fn extract_correlation_id_from_http_request_without_correlation_id() {
        let http_request = TestRequest::default().to_http_request();

        http_request.correlation_id();
    }

    #[test]
    fn extract_correlation_id_from_http_request_with_correlation_id() {
        let correlation_id = UuidCorrelationIdGenerator
            .generate_correlation_id()
            .unwrap();
        let http_request = TestRequest::default().to_http_request();
        http_request.extensions_mut().insert(correlation_id.clone());

        assert_eq!(http_request.correlation_id(), correlation_id);
    }
}
