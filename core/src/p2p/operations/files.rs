use std::path::PathBuf;

use sd_p2p::{RemoteIdentity, UnicastStream};
use tokio::io::AsyncRead;
use tracing::debug;

/// Request a file from a remote peer
#[allow(unused)]
pub async fn request(remote: RemoteIdentity, path: PathBuf) -> Result<AsyncRead, ()> {
	todo!();
}

pub(crate) async fn receiver(stream: UnicastStream) {
	// TODO: Apply security rules around exposing files

	// debug!("Received ping from peer '{}'", stream.remote_identity());

	// TODO: File progress
}
