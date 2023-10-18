use std::sync::Arc;

use crate::p2p::{Header, P2PManager};

/// Send a ping to all peers we are connected to
pub async fn ping(p2p: Arc<P2PManager>) {
	p2p.manager.broadcast(Header::Ping.to_bytes()).await;
}
