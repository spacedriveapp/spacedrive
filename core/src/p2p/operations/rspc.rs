use std::{error::Error, sync::Arc};

use axum::{extract::Request, http, Router};
use hyper::{body::Incoming, client::conn::http1::handshake, server::conn::http1, Response};
use hyper_util::rt::TokioIo;
use sd_p2p::{RemoteIdentity, UnicastStream, P2P};
use tokio::io::AsyncWriteExt;
use tower_service::Service;
use tracing::debug;

use crate::{p2p::Header, Node};

/// Transfer an rspc query to a remote node.
pub async fn remote_rspc(
	p2p: Arc<P2P>,
	identity: RemoteIdentity,
	request: http::Request<axum::body::Body>,
) -> Result<Response<Incoming>, Box<dyn Error>> {
	let peer = p2p
		.peers()
		.get(&identity)
		.ok_or("Peer not found, has it been discovered?")?
		.clone();
	let mut stream = peer.new_stream().await?;

	stream.write_all(&Header::RspcRemote.to_bytes()).await?;

	let (mut sender, conn) = handshake(TokioIo::new(stream)).await?;
	tokio::task::spawn(async move {
		if let Err(e) = conn.await {
			println!("Connection error: {:?}", e);
		}
	});

	sender.send_request(request).await.map_err(Into::into)
}

pub(crate) async fn receiver(
	stream: UnicastStream,
	service: &mut Router,
	node: &Node,
) -> Result<(), Box<dyn Error>> {
	debug!(
		peer = %stream.remote_identity(),
		"Received http request from;",
	);

	// TODO: Authentication
	#[allow(clippy::todo)]
	if !node.config.get().await.p2p.enable_remote_access {
		todo!("No way buddy!");
	}

	let hyper_service =
		hyper::service::service_fn(move |request: Request<Incoming>| service.clone().call(request));

	http1::Builder::new()
		.keep_alive(true)
		.serve_connection(TokioIo::new(stream), hyper_service)
		.with_upgrades()
		.await
		.map_err(Into::into)
}
