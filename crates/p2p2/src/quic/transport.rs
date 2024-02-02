use std::{convert::Infallible, sync::Arc};

use libp2p::{
	core::{muxing::StreamMuxerBox, transport::ListenerId},
	SwarmBuilder, Transport,
};
use tokio::sync::mpsc;

use crate::{p2p::HookId, P2P};

// TODO: Register as peer connection method and allow connecting

/// Transport using Quic to establish a connection between peers.
/// This uses `libp2p` internally.
#[derive(Debug)]
pub struct QuicTransport {
	p2p: Arc<P2P>,
	hook_id: HookId,
	// ipv4_listener_id: Option<Result<ListenerId, String>>,
	// ipv4_port: Option<u16>,
	// ipv6_listener_id: Option<Result<ListenerId, String>>,
	// ipv6_port: Option<u16>,
	// // A map of connected clients.
	// // This includes both inbound and outbound connections!
	// pub(crate) connected: HashMap<libp2p::PeerId, RemoteIdentity>,
}

impl QuicTransport {
	pub fn spawn(p2p: Arc<P2P>) -> Result<Self, ()> {
		let keypair: libp2p::identity::Keypair = todo!(); // TODO: Work out how to do this conversion

		let (tx, rx) = mpsc::channel(15);
		let hook_id = p2p.register_hook(tx);

		// p2p.listeners_mut().insert(k, Listener::new()); // TODO: These are important
		// TODO: Cleanup listeners on shutdown

		// let application_name = format!("/{application_name}/spacetime/1.0.0");
		// stream_id: AtomicU64::new(0),

		// let mut swarm = ok(ok(SwarmBuilder::with_existing_identity(keypair)
		// 	.with_tokio()
		// 	.with_other_transport(|keypair| {
		// 		libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
		// 			libp2p_quic::Config::new(keypair),
		// 		)
		// 		.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
		// 		.boxed()
		// 	}))
		// .with_behaviour(|_| SpaceTime::new(this.clone())))
		// .build();

		Ok(Self { p2p, hook_id })
	}

	// TODO: User can set port
	// TODO: Enabled/disabled specific listener
	// TODO: Get the listeners and `peer_id` from `sd-core`

	pub fn shutdown(self) {
		self.p2p.unregister_hook(self.hook_id);
	}
}

fn ok<T>(v: Result<T, Infallible>) -> T {
	match v {
		Ok(v) => v,
		Err(_) => unreachable!(),
	}
}
