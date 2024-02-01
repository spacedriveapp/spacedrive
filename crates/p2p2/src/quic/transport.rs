use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{p2p::HookId, P2P};

// TODO: Register as peer connection method and allow connecting

/// Transport using Quic to establish a connection between peers.
/// This uses `libp2p` internally.
#[derive(Debug)]
pub struct QuicTransport {
	p2p: Arc<P2P>,
	hook_id: HookId,
}

impl QuicTransport {
	pub fn spawn(p2p: Arc<P2P>) -> Result<Self, ()> {
		let identity: libp2p::identity::Keypair = todo!(); // TODO: Work out how to do this conversion

		let (tx, rx) = mpsc::channel(15);
		let hook_id = p2p.register_hook(tx);

		// start(p2p.clone(), rx)?;

		Ok(Self { p2p, hook_id })
	}

	// TODO: User can set port

	pub fn shutdown(self) {
		self.p2p.unregister_hook(self.hook_id);
	}
}

// pub(crate) ipv4_listener_id: Option<Result<ListenerId, String>>,
// pub(crate) ipv4_port: Option<u16>,
// pub(crate) ipv6_listener_id: Option<Result<ListenerId, String>>,
// pub(crate) ipv6_port: Option<u16>,
// // A map of connected clients.
// // This includes both inbound and outbound connections!
// pub(crate) connected: HashMap<libp2p::PeerId, RemoteIdentity>,
