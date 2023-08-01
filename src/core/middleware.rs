use std::{
    future::{ready, Ready},
    rc::Rc,
    task::{Context, Poll},
};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::{ErrorBadRequest, ErrorInternalServerError},
    http::header::{HeaderName, HeaderValue},
    Error, HttpMessage,
};
use futures::{
    future::{Either, LocalBoxFuture},
    FutureExt,
};

use crate::{Config, CorrelationId, CorrelationIdExtract, CorrelationIdGenerator};

pub struct Correlation {
    config: Rc<Config>,
}

impl Correlation {
    /// Sets the name of the header from which the Correlation ID is read from the request.
    pub fn request_header_name<T>(mut self, header_name: T) -> Self
    where
        T: Into<HeaderName>,
    {
        self.modify_config(|cfg| cfg.header_name = header_name.into());
        self
    }

    fn modify_config<M>(&mut self, modification: M)
    where
        M: FnOnce(&mut Config),
    {
        if let Some(cfg) = Rc::get_mut(&mut self.config) {
            modification(cfg);
        }
    }

    /// Enforce the inclusion of the correlation ID request header.
    ///
    /// If `true` and the supposed correlation ID header is not included, the
    /// request will fail with a 400 Bad Request response.
    pub fn enforce_request_header(mut self, enforce: bool) -> Self {
        self.modify_config(|cfg| cfg.enforce_header = enforce);
        self
    }

    /// The name of the header to which the correlation ID is written for the response.
    pub fn response_header_name<T>(mut self, header_name: T) -> Self
    where
        T: Into<HeaderName>,
    {
        self.modify_config(|cfg| cfg.resp_header_name = header_name.into());
        self
    }

    /// Controls whether the correlation ID is returned in the response headers.
    pub fn include_in_response(mut self, include_in_response: bool) -> Self {
        self.modify_config(|cfg| cfg.include_in_resp = include_in_response);
        self
    }

    /// Use the provided generator for creating a `CorrelationId` instead of
    /// the default one.
    pub fn with_id_generator(mut self, id_generator: Box<dyn CorrelationIdGenerator>) -> Self {
        self.modify_config(|cfg| cfg.correlation_id_generator = id_generator);
        self
    }
}

impl Default for Correlation {
    /// Creates the default instance of `Correlation` with the following configuration:
    ///
    /// * request header name: `"x-correlation-id"`,
    /// * enforce request header: `false`,
    /// * response header name: `"x-correlation-id"`,
    /// * include in response: `true`,
    /// * ID generator: simple UUID (v4).
    fn default() -> Self {
        Self {
            config: Rc::new(Config::default()),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for Correlation
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = CorrelationMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CorrelationMiddleware {
            service,
            config: Rc::clone(&self.config),
        }))
    }
}

pub struct CorrelationMiddleware<S> {
    service: S,
    config: Rc<Config>,
}

impl<S, B> Service<ServiceRequest> for CorrelationMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Either<
        Ready<Result<ServiceResponse<B>, Error>>,
        LocalBoxFuture<'static, Result<ServiceResponse<B>, Error>>,
    >;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let correlation_id = match try_obtain_correlation_id(self.config.clone(), &request) {
            Ok(correlation_id) => correlation_id,
            Err(e) => return Either::Left(ready(Err(e))),
        };

        request.extensions_mut().insert(correlation_id);

        let fut = self.service.call(request);
        let config = Rc::clone(&self.config);

        Either::Right(
            async move {
                let mut response = fut.await?;

                if config.include_in_resp {
                    let correlation_id = response.request().correlation_id();

                    response.headers_mut().insert(
                        config.resp_header_name.clone(),
                        HeaderValue::from_str(&correlation_id).unwrap(),
                    );
                }

                Ok(response)
            }
            .boxed_local(),
        )
    }
}

fn try_obtain_correlation_id(
    config: Rc<Config>,
    req: &ServiceRequest,
) -> Result<CorrelationId, Error> {
    let header_name = &config.header_name;
    match req.headers().get(header_name) {
        Some(header_value) => try_header_value_to_correlation_id(header_name, header_value),
        None => {
            if config.enforce_header {
                Err(ErrorBadRequest(format!(
                    "header '{header_name}' is required"
                )))
            } else {
                try_generate_correlation_id(&*config.correlation_id_generator)
            }
        }
    }
}

fn try_header_value_to_correlation_id(
    header_name: &HeaderName,
    header_value: &HeaderValue,
) -> Result<CorrelationId, Error> {
    match header_value.to_str() {
        Ok(header_value_str) => match header_value_str.parse::<CorrelationId>() {
            Ok(correlation_id) => Ok(correlation_id),
            Err(e) => Err(ErrorBadRequest(e.to_string())),
        },
        Err(_) => Err(ErrorBadRequest(format!(
            "value of header '{header_name}' contains non-visible ASCII chars"
        ))),
    }
}

fn try_generate_correlation_id(
    correlation_id_generator: &dyn CorrelationIdGenerator,
) -> Result<CorrelationId, Error> {
    correlation_id_generator
        .generate_correlation_id()
        .map_err(|e| ErrorInternalServerError(e.to_string()))
}

#[cfg(test)]
mod correlation_tests {
    use actix_web::http::header::HeaderName;

    use crate::Correlation;

    #[test]
    fn test_default_correlation_config() {
        let correlation = Correlation::default();
        let default_config = correlation.config;

        assert_eq!(
            HeaderName::from_static("x-correlation-id"),
            default_config.header_name
        );
        assert!(!default_config.enforce_header);
        assert_eq!(
            HeaderName::from_static("x-correlation-id"),
            default_config.resp_header_name
        );
        assert!(default_config.include_in_resp);
    }

    #[test]
    fn test_set_request_header_name() {
        let header_name_str = "my-corr-id";
        let mut correlation = Correlation::default();
        correlation = correlation.request_header_name(HeaderName::from_static(header_name_str));

        assert_eq!(header_name_str, correlation.config.header_name.as_str());
    }

    #[test]
    fn test_set_enforce_request_header_to_true() {
        let mut correlation = Correlation::default();
        correlation = correlation.enforce_request_header(true);

        assert!(correlation.config.enforce_header);
    }

    #[test]
    fn test_set_response_header_name() {
        let header_name_str = "x-transaction-id";
        let mut correlation = Correlation::default();
        correlation = correlation.response_header_name(HeaderName::from_static(header_name_str));

        assert_eq!(
            header_name_str,
            correlation.config.resp_header_name.as_str()
        );
    }

    #[test]
    fn test_set_include_in_response_to_false() {
        let mut correlation = Correlation::default();
        correlation = correlation.include_in_response(false);

        assert!(!correlation.config.include_in_resp);
    }
}
