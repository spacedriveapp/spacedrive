use sd_p2p::UnicastStream;
use tracing::debug;

/// Send a ping to all peers we are connected to
#[allow(unused)]
pub async fn ping() {
	todo!();
}

pub(crate) async fn receiver(stream: UnicastStream) {
	debug!(peer = %stream.remote_identity(), "Received ping from;");
}
