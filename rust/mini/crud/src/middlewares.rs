use axum::{body::Body, http::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn print_request_info(req: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let uri = req.uri().to_string();

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    println!("{} {} > {} ({:?})", method, uri, status, duration);

    response
}
