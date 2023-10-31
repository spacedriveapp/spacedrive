use std::sync::Arc;

use sd_p2p::PeerMessageEvent;
use tracing::debug;

use crate::p2p::P2PManager;

/// Send a ping to all peers we are connected to
pub async fn ping(_p2p: Arc<P2PManager>) {
	// p2p.manager.broadcast(Header::Ping.to_bytes()).await;
	todo!();
}

pub(crate) async fn reciever(event: PeerMessageEvent) {
	debug!("Received ping from peer '{}'", event.identity);
}
