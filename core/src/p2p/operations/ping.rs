use std::{error::Error, sync::Arc};

use sd_p2p::{RemoteIdentity, UnicastStream, P2P};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::debug;

use crate::p2p::Header;

/// Send a ping to all peers we are connected to
#[allow(unused)]
pub async fn ping(p2p: Arc<P2P>, identity: RemoteIdentity) -> Result<(), Box<dyn Error>> {
	let peer = p2p
		.peers()
		.get(&identity)
		.ok_or("Peer not found, has it been discovered?")?
		.clone();
	let mut stream = peer.new_stream().await?;

	stream.write_all(&Header::Ping.to_bytes()).await?;

	let mut result = [0; 4];
	let _ = stream.read_exact(&mut result).await?;
	if result != *b"PONG" {
		return Err("Failed to receive pong".into());
	}

	Ok(())
}

pub(crate) async fn receiver(mut stream: UnicastStream) {
	debug!(peer = %stream.remote_identity(), "Received ping from;");

	stream
		.write_all(b"PONG")
		.await
		.expect("Failed to send pong");
}
