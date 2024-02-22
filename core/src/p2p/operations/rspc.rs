use axum::http::{self, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use sd_p2p2::{RemoteIdentity, UnicastStream};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
	#[serde(with = "http_serde::method")]
	pub method: Method,
	#[serde(with = "http_serde::uri")]
	pub uri: Uri,
	#[serde(with = "http_serde::header_map")]
	pub headers: HeaderMap<HeaderValue>,
	pub body: Vec<u8>,
}

impl Request {
	pub fn into_req(self) -> http::Request<Vec<u8>> {
		let mut req = http::Request::new(self.body);
		*req.method_mut() = self.method;
		*req.uri_mut() = self.uri;
		*req.headers_mut() = self.headers;
		req
	}
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
	#[serde(with = "http_serde::status_code")]
	pub status: StatusCode,
	#[serde(with = "http_serde::header_map")]
	pub headers: HeaderMap<HeaderValue>,
	pub body: Vec<u8>,
}

/// Transfer an rspc query to a remote node.
#[allow(unused)]
pub async fn remote_rspc(identity: RemoteIdentity, req: http::Request<()>) {
	todo!();
}
