use std::{fmt::Debug, panic::Location};

use axum::{
	body::{self, BoxBody},
	http::{self, HeaderValue, Method, Request, Response, StatusCode},
	middleware::Next,
};
use http_body::Full;
use tracing::debug;

use crate::util::InfallibleResponse;

#[track_caller]
pub(crate) fn bad_request(err: impl Debug) -> http::Response<BoxBody> {
	debug!("400: Bad Request at {}: {err:?}", Location::caller());

	InfallibleResponse::builder()
		.status(StatusCode::BAD_REQUEST)
		.body(body::boxed(Full::from("")))
}

#[track_caller]
pub(crate) fn not_found(err: impl Debug) -> http::Response<BoxBody> {
	debug!("404: Not Found at {}: {err:?}", Location::caller());

	InfallibleResponse::builder()
		.status(StatusCode::NOT_FOUND)
		.body(body::boxed(Full::from("")))
}

#[track_caller]
pub(crate) fn internal_server_error(err: impl Debug) -> http::Response<BoxBody> {
	debug!(
		"500 - Internal Server Error at {}: {err:?}",
		Location::caller()
	);

	InfallibleResponse::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
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

	let mut response = next.run(req).await;

	{
		let headers = response.headers_mut();

		headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));

		headers.insert(
			"Access-Control-Allow-Headers",
			HeaderValue::from_static("*"),
		);

		// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection
		headers.insert("Connection", HeaderValue::from_static("Keep-Alive"));

		headers.insert("Server", HeaderValue::from_static("Spacedrive"));
	}

	response
}
