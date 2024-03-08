use sd_p2p2::UnicastStream;
use tracing::debug;

/// Send a ping to all peers we are connected to
#[allow(unused)]
pub async fn ping() {
	todo!();
}

pub(crate) async fn receiver(stream: UnicastStream) {
	debug!("Received ping from peer '{}'", stream.remote_identity());
}
