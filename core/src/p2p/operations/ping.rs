use std::sync::Arc;

use sd_p2p::{spacetime::UnicastStream, PeerMessageEvent};
use tracing::debug;

use crate::p2p::{Header, P2PManager};

/// Send a ping to all peers we are connected to
pub async fn ping(p2p: Arc<P2PManager>) {
	p2p.manager.broadcast(Header::Ping.to_bytes()).await;
}

pub(crate) async fn reciever(event: PeerMessageEvent<UnicastStream>) {
	debug!("Received ping from peer '{}'", event.identity);
}
