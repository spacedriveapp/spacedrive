use crate::util::InfallibleResponse;

use std::{fmt::Debug, panic::Location};

use axum::{
	body::{self, BoxBody},
	http::{self, HeaderValue, Method, Request, Response, StatusCode},
	middleware::Next,
};
use http_body::Full;
use tracing::debug;

#[track_caller]
pub(crate) fn bad_request(e: impl Debug) -> http::Response<BoxBody> {
	debug!(caller = %Location::caller(), ?e, "400: Bad Request;");

	InfallibleResponse::builder()
		.status(StatusCode::BAD_REQUEST)
		.body(body::boxed(Full::from("")))
}

#[track_caller]
pub(crate) fn not_found(e: impl Debug) -> http::Response<BoxBody> {
	debug!(caller = %Location::caller(), ?e, "404: Not Found;");

	InfallibleResponse::builder()
		.status(StatusCode::NOT_FOUND)
		.body(body::boxed(Full::from("")))
}

#[track_caller]
pub(crate) fn internal_server_error(e: impl Debug) -> http::Response<BoxBody> {
	debug!(caller = %Location::caller(), ?e, "500: Internal Server Error;");

	InfallibleResponse::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.body(body::boxed(Full::from("")))
}

#[track_caller]
pub(crate) fn not_implemented(e: impl Debug) -> http::Response<BoxBody> {
	debug!(caller = %Location::caller(), ?e, "501: Not Implemented;");

	InfallibleResponse::builder()
		.status(StatusCode::NOT_IMPLEMENTED)
		.body(body::boxed(Full::from("")))
}

pub(crate) async fn cors_middleware<B>(req: Request<B>, next: Next<B>) -> Response<BoxBody> {
	if req.method() == Method::OPTIONS {
		return Response::builder()
			.header("Access-Control-Allow-Methods", "GET, HEAD, POST, OPTIONS")
			.header("Access-Control-Allow-Origin", "*")
			.header("Access-Control-Allow-Headers", "*")
			.header("Access-Control-Max-Age", "86400")
			.status(StatusCode::OK)
			.body(body::boxed(Full::from("")))
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
