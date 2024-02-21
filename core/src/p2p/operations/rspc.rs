use sd_p2p2::{RemoteIdentity, UnicastStream};
use tracing::debug;

/// Transfer an rspc query to a remote node.
#[allow(unused)]
pub async fn remote_rspc(identity: RemoteIdentity) {
	todo!();
}

pub(crate) async fn reciever(stream: UnicastStream) {
	debug!(
		"Received rspc request from peer '{}'",
		stream.remote_identity()
	);

	todo!();
}
