use crate::util::InfallibleResponse;

use std::{fmt::Debug, panic::Location};

use axum::{
	body::Body,
	http::{self, HeaderValue, Method, Request, Response, StatusCode},
	middleware::Next,
};
use tracing::debug;

#[track_caller]
pub(crate) fn bad_request(e: impl Debug) -> http::Response<Body> {
	debug!(caller = %Location::caller(), ?e, "400: Bad Request;");

	InfallibleResponse::builder()
		.status(StatusCode::BAD_REQUEST)
		.body(Body::from(""))
}

#[track_caller]
pub(crate) fn not_found(e: impl Debug) -> http::Response<Body> {
	debug!(caller = %Location::caller(), ?e, "404: Not Found;");

	InfallibleResponse::builder()
		.status(StatusCode::NOT_FOUND)
		.body(Body::from(""))
}

#[track_caller]
pub(crate) fn internal_server_error(e: impl Debug) -> http::Response<Body> {
	debug!(caller = %Location::caller(), ?e, "500: Internal Server Error;");

	InfallibleResponse::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.body(Body::from(""))
}

#[track_caller]
pub(crate) fn not_implemented(e: impl Debug) -> http::Response<Body> {
	debug!(caller = %Location::caller(), ?e, "501: Not Implemented;");

	InfallibleResponse::builder()
		.status(StatusCode::NOT_IMPLEMENTED)
		.body(Body::from(""))
}

pub(crate) async fn cors_middleware(req: Request<Body>, next: Next) -> Response<Body> {
	if req.method() == Method::OPTIONS {
		return Response::builder()
			.header("Access-Control-Allow-Methods", "GET, HEAD, POST, OPTIONS")
			.header("Access-Control-Allow-Origin", "*")
			.header("Access-Control-Allow-Headers", "*")
			.header("Access-Control-Max-Age", "86400")
			.status(StatusCode::OK)
			.body(Body::from(""))
			.expect("Invalid static response!");
	}

	let is_upgrade_request = req.headers().get("Upgrade").is_some();

	let mut response = next.run(req).await;

	{
		let headers = response.headers_mut();

		headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));

		headers.insert(
			"Access-Control-Allow-Headers",
			HeaderValue::from_static("*"),
		);

		// With websocket requests, setting this causes the browser to loose it's shit.
		if !is_upgrade_request {
			// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection
			headers.insert("Connection", HeaderValue::from_static("Keep-Alive"));
		}

		headers.insert("Server", HeaderValue::from_static("Spacedrive"));
	}

	response
}
