use actix_web::{
    dev::ServiceResponse,
    http::{
        header::{AsHeaderName, ContentType, HeaderName},
        Error, StatusCode,
    },
    test::{self, TestRequest},
    web::{self, Bytes},
    App, HttpResponse, Route,
};
use actix_web_correlation_id::{Correlation, CorrelationId, CorrelationIdGenerator};

static DEFAULT_HEADER_NAME: HeaderName = HeaderName::from_static("x-correlation-id");

trait BodyTest {
    fn as_str(&self) -> &str;
}

impl BodyTest for Bytes {
    fn as_str(&self) -> &str {
        std::str::from_utf8(self).unwrap()
    }
}

struct TestRoute<'a> {
    path: &'a str,
    route: Route,
}

impl<'a> Default for TestRoute<'a> {
    fn default() -> Self {
        Self {
            path: "/",
            route: web::get().to(respond_with_correlation_id_in_body),
        }
    }
}

struct StaticCorrelationidGenerator;

impl CorrelationIdGenerator for StaticCorrelationidGenerator {
    fn generate_correlation_id(
        &self,
    ) -> Result<CorrelationId, actix_web_correlation_id::CorrelationIdError> {
        CorrelationId::try_from("YOLO!".to_string())
    }
}

async fn respond_with_correlation_id_in_body(
    correlation_id: CorrelationId,
) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .insert_header(ContentType::plaintext())
        .body(correlation_id.to_string()))
}

#[actix_web::test]
async fn correlation_id_gets_extracted_from_request() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default())
            .route(test_route.path, test_route.route),
    )
    .await;
    let correlation_id_value = "abc123";
    let req = TestRequest::get()
        .uri(test_route.path)
        .insert_header((DEFAULT_HEADER_NAME.as_str(), correlation_id_value))
        .to_request();
    let body = test::call_and_read_body(&app, req).await;

    assert_eq!(body.as_str(), correlation_id_value);
}

#[actix_web::test]
async fn correlation_id_gets_inserted_into_response() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default())
            .route(test_route.path, test_route.route),
    )
    .await;
    let correlation_id_value = "fajfkaefiaefaefag";
    let req = TestRequest::get()
        .uri(test_route.path)
        .insert_header((DEFAULT_HEADER_NAME.as_str(), correlation_id_value))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(
        correlation_id_value,
        correlation_id_from_headers(&resp, DEFAULT_HEADER_NAME.clone()).unwrap()
    );
}

fn correlation_id_from_headers(
    resp: &ServiceResponse,
    header_name: impl AsHeaderName,
) -> Option<&str> {
    resp.headers()
        .get(header_name)
        .and_then(|header_value| header_value.to_str().ok())
}

#[actix_web::test]
async fn generate_correlation_id_if_absent_in_request_headers() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default())
            .route(test_route.path, test_route.route),
    )
    .await;
    let req = TestRequest::get().uri(test_route.path).to_request();
    let resp = test::call_service(&app, req).await;

    match correlation_id_from_headers(&resp, DEFAULT_HEADER_NAME.clone()) {
        Some(correlation_id) => assert!(!correlation_id.is_empty()),
        None => panic!("expected a correlation ID in response headers but got none"),
    }
}

#[actix_web::test]
async fn omit_correlation_id_from_response() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default().include_in_response(false))
            .route(test_route.path, test_route.route),
    )
    .await;
    let correlation_id_value = "fajfkaefiaefaefag";
    let req = TestRequest::get()
        .uri(test_route.path)
        .insert_header((DEFAULT_HEADER_NAME.as_str(), correlation_id_value))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let correlation_id_from_headers =
        correlation_id_from_headers(&resp, DEFAULT_HEADER_NAME.clone());

    assert!(correlation_id_from_headers.is_none())
}

#[actix_web::test]
async fn enforce_correlation_id_request_header() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default().enforce_request_header(true))
            .route(test_route.path, test_route.route),
    )
    .await;
    let req = TestRequest::get().uri(test_route.path).to_request();
    let result = test::try_call_service(&app, req).await;

    match result {
        Ok(_) => panic!("expected an error but got a response"),
        Err(e) => assert_eq!("header 'x-correlation-id' is required", e.to_string()),
    }
}

#[actix_web::test]
async fn use_custom_correlation_id_generator() {
    let test_route = TestRoute::default();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default().with_id_generator(Box::new(StaticCorrelationidGenerator)))
            .route(test_route.path, test_route.route),
    )
    .await;
    let req = TestRequest::get().uri(test_route.path).to_request();
    let resp = test::call_service(&app, req).await;

    match correlation_id_from_headers(&resp, DEFAULT_HEADER_NAME.clone()) {
        Some(correlation_id) => assert_eq!("YOLO!", correlation_id),
        None => panic!("expected a correlation ID in response headers but got none"),
    }
}

#[actix_web::test]
async fn send_invalid_correlation_id_in_request_header() {
    let test_route = TestRoute::default();
    let correlation_id_value = "asdfjklö";
    let app = actix_web::test::init_service(
        App::new()
            .wrap(Correlation::default())
            .route(test_route.path, test_route.route),
    )
    .await;
    let req = TestRequest::get()
        .uri(test_route.path)
        .insert_header((DEFAULT_HEADER_NAME.clone(), correlation_id_value))
        .to_request();
    let result = test::try_call_service(&app, req).await;

    match result {
        Ok(_) => panic!("expected an error but got a response"),
        Err(e) => assert_eq!(
            format!("value of header '{DEFAULT_HEADER_NAME}' contains non-visible ASCII chars"),
            e.to_string()
        ),
    }
}
