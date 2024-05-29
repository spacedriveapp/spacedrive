use std::{error::Error, sync::Arc};

use sd_p2p::{RemoteIdentity, UnicastStream, P2P};
use tokio::io::AsyncWriteExt;
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

	stream.write_all(&Header::Http.to_bytes()).await?;

	Ok(())
}

pub(crate) async fn receiver(stream: UnicastStream) {
	debug!("Received ping from peer '{}'", stream.remote_identity());
}
