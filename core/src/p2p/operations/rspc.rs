use std::{convert::Infallible, error::Error, fmt, sync::Arc};

use axum::{
	body::Body,
	http::{self, HeaderMap, HeaderValue, Method, StatusCode, Uri},
	Router,
};
use http_body::Body as _;
use sd_p2p2::{RemoteIdentity, UnicastStream, P2P};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower_service::Service;
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
pub async fn remote_rspc<B: http_body::Body>(
	p2p: Arc<P2P>,
	identity: RemoteIdentity,
	req: http::Request<B>,
) -> Result<Response, Box<dyn Error>>
where
	B::Error: fmt::Debug + Error + 'static,
{
	let peer = p2p
		.peers()
		.get(&identity)
		.ok_or("Peer not found, has it been discovered?")?
		.clone();
	let mut stream = peer.new_stream().await?;

	let req = Request {
		method: req.method().clone(),
		uri: req.uri().clone(),
		headers: req.headers().clone(),
		body: req.into_body().collect().await?.to_bytes().to_vec(),
	};

	stream.write_all(&Header::Rspc(req).to_bytes()).await?;

	let status = stream.read_u8().await?;
	if status != 0 {
		return Err("Received error status from remote rspc query".into());
	}

	let len = stream.read_u64_le().await?;

	let mut buf = vec![0; len as usize];
	stream.read_exact(&mut buf).await?;

	rmp_serde::from_read(&*buf).map_err(Into::into)
}

pub(crate) async fn receiver(
	mut stream: UnicastStream,
	req: Request,
	service: &mut Router,
) -> Result<(), Box<dyn Error>> {
	debug!(
		"Received rspc request from peer '{}': {} {}",
		stream.remote_identity(),
		req.method,
		req.uri
	);

	let res = unwrap_infallible(service.call(req.into_req().map(Body::from)).await);
	let result = Response {
		status: res.status(),
		headers: res.headers().clone(),
		body: match res.into_body().collect().await {
			Ok(b) => b.to_bytes().to_vec(),
			Err(e) => {
				stream.write_u8(1).await.ok();
				return Err(e.into());
			}
		},
	};

	let buf = match rmp_serde::to_vec(&result) {
		Ok(buf) => buf,
		Err(e) => {
			stream.write_u8(1).await.ok();
			return Err(e.into());
		}
	};
	stream.write_u8(0).await?;
	stream.write_all(&(buf.len() as u64).to_le_bytes()).await?;
	stream.write_all(&buf).await?;

	Ok(())
}

pub(crate) fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
	match result {
		Ok(value) => value,
		Err(err) => match err {},
	}
}
