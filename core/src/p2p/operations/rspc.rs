use std::{fmt, sync::Arc};

use axum::http::{self, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use http_body::Body;
use sd_p2p2::{RemoteIdentity, UnicastStream, P2P};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

use crate::p2p::Header;

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
pub async fn remote_rspc<B: Body>(
	p2p: Arc<P2P>,
	identity: RemoteIdentity,
	req: http::Request<B>,
) -> Result<Response, ()>
where
	B::Error: fmt::Debug,
{
	let peer = p2p.peers().get(&identity).unwrap().clone(); // TODO: error handling
	let mut stream = peer.new_stream().await.unwrap(); // TODO: error handling

	let req = Request {
		method: req.method().clone(),
		uri: req.uri().clone(),
		headers: req.headers().clone(),
		body: req.into_body().collect().await.unwrap().to_bytes().to_vec(), // TODO: error handling
	};

	stream
		.write_all(&Header::Rspc(req).to_bytes())
		.await
		.unwrap(); // TODO: error handling

	let len = stream.read_u64_le().await.unwrap(); // TODO: error handling

	let mut buf = vec![0; len as usize];
	stream.read_exact(&mut buf).await.unwrap(); // TODO: error handling

	let resp: Response = rmp_serde::from_read(&*buf).unwrap(); // TODO: error handling

	debug!("Received rspc response: {:?}", resp); // TODO

	Ok(resp)
}
