use std::{
    borrow::Cow,
    ops::Deref,
    rc::Rc,
    task::{Context, Poll},
};

use actix_service::{Service, Transform};
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest, dev::{Payload, ServiceRequest, ServiceResponse}, error::{ErrorBadRequest, ErrorInternalServerError}};
use futures::{FutureExt, future::{Either, LocalBoxFuture, Ready, err, ok}};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CorrelationId {
    key: String,
    value: String,
}

impl CorrelationId {
    pub fn get_key(&self) -> &str {
        &self.key
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }
}

impl Deref for CorrelationId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl FromRequest for CorrelationId {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;
    type Config = ();

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        if let Some(s) = req.extensions().get::<CorrelationId>() {
            ok(s.clone())
        } else {
            println!("use correlation middleware in pipeline");
            err(ErrorInternalServerError("internal server error"))
        }
    }
}

struct Config {
    header_name: String,
    enforce_header: bool,
    resp_header_name: Option<String>,
    include_in_resp: bool,
}

pub struct Correlation {
    config: Rc<Config>,
}

impl Correlation {
    pub fn new() -> Self {
        Self {
            config: Rc::new(Config {
                header_name: "x-correlation-id".into(),
                enforce_header: false,
                resp_header_name: None,
                include_in_resp: true,
            }),
        }
    }

    /// The name of the header from which the Correlation ID is read from the request
    pub fn header_name(mut self, v: String) -> Self {
        Rc::get_mut(&mut self.config).unwrap().header_name = v;
        self
    }

    /// Enforce the inclusion of the correlation ID request header.
    /// When true and a correlation ID header is not included, the request will fail with a 400 Bad Request response
    pub fn enforce_header(mut self, v: bool) -> Self {
        Rc::get_mut(&mut self.config).unwrap().enforce_header = v;
        self
    }

    /// The name of the header to which the Correlation ID is written for the response
    pub fn resp_header_name(mut self, v: Option<String>) -> Self {
        Rc::get_mut(&mut self.config).unwrap().resp_header_name = v;
        self
    }

    /// Controls whether the correlation ID is returned in the response headers
    pub fn include_in_resp(mut self, v: bool) -> Self {
        Rc::get_mut(&mut self.config).unwrap().include_in_resp = v;
        self
    }
}

impl<S, B> Transform<S> for Correlation
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = Middleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(Middleware {
            service,
            config: Rc::clone(&self.config),
        })
    }
}

pub struct Middleware<S> {
    service: S,
    config: Rc<Config>,
}

impl<S, B> Service for Middleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Either<Ready<Result<Self::Response, Self::Error>>, LocalBoxFuture<'static, Result<Self::Response, Self::Error>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let value = match req.headers().get(&self.config.header_name) {
            Some(v) => v.to_str().unwrap().to_owned(),
            None => {
                if self.config.enforce_header {
                    return Either::Left(err(ErrorBadRequest(format!(
                        "Header '{}' is required",
                        self.config.header_name
                    ))));
                } else {
                    gen_corr_id()
                }
            }
        };

        let corr_id = CorrelationId {
            key: self.config.header_name.to_owned(),
            value: value,
        };

        req.extensions_mut().insert(corr_id);

        let fut = self.service.call(req);
        let config = Rc::clone(&self.config);

        Either::Right(async move {
            let resp = fut.await;

            if config.include_in_resp {
                let name = match config.resp_header_name {
                    Some(ref s) => s,
                    None => &config.header_name
                };


            }

            resp
        }.boxed_local())
    }
}

fn gen_corr_id() -> String {
    Uuid::new_v4().to_simple().to_string()
}
