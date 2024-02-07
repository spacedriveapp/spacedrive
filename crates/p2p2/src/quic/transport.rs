use std::{
	convert::Infallible,
	net::{SocketAddrV4, SocketAddrV6},
	sync::{Arc, RwLock},
};

use flume::{bounded, Receiver};
use libp2p::{core::muxing::StreamMuxerBox, futures::StreamExt, Swarm, SwarmBuilder, Transport};

use crate::{HookEvent, HookId, ListenerId, P2P};

use super::behaviour::SpaceTime;

// TODO: User can set port
// TODO: Enabled/disabled specific listener
// TODO: Get the listeners and `peer_id` from `sd-core`
// TODO: Ensure we remove all information from the `Peer` when they disconnect

/// [libp2p::PeerId] for debugging purposes only.
#[derive(Debug)]
pub struct Libp2pPeerId(libp2p::PeerId);

/// Transport using Quic to establish a connection between peers.
/// This uses `libp2p` internally.
#[derive(Debug)]
pub struct QuicTransport {
	id: ListenerId,
	p2p: Arc<P2P>,
	state: Arc<RwLock<State>>,
}

#[derive(Debug, Default)]
struct State {
	ipv4_addr: Option<Listener<SocketAddrV4>>,
	ipv6_addr: Option<Listener<SocketAddrV6>>,
}

#[derive(Debug)]
struct Listener<T> {
	addr: T,
	libp2p: Result<ListenerId, String>,
}

impl QuicTransport {
	/// Spawn the `QuicTransport` and register it with the P2P system.
	/// Be aware spawning this does nothing unless you call `Self::set_ipv4_enabled`/`Self::set_ipv6_enabled` to enable the listeners.
	// TODO: Error type here
	pub fn spawn(p2p: Arc<P2P>) -> Result<(Self, Libp2pPeerId), String> {
		// This is sketchy, but it makes the whole system a lot easier to work with
		// We are assuming the libp2p `Keypair`` is the same format as our `Identity` type.
		// This is *acktually* true but they reserve the right to change it at any point.
		let keypair =
			libp2p::identity::Keypair::ed25519_from_bytes(p2p.identity().to_bytes()).unwrap(); // TODO: Work out how to do this conversion
		let libp2p_peer_id = Libp2pPeerId(keypair.public().to_peer_id());

		let (tx, rx) = bounded(15);
		let id = p2p.register_listener("libp2p-quic", tx, |peer, addrs| {
			todo!();
		});

		let application_name = format!("/{}/spacetime/1.0.0", p2p.app_name());

		let mut swarm = ok(ok(SwarmBuilder::with_existing_identity(keypair)
			.with_tokio()
			.with_other_transport(|keypair| {
				libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
					libp2p_quic::Config::new(keypair),
				)
				.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
				.boxed()
			}))
		.with_behaviour(|_| SpaceTime::new(p2p.clone(), id)))
		.build();

		let state: Arc<RwLock<State>> = Default::default();
		tokio::spawn(start(p2p.clone(), state.clone(), swarm, rx));

		Ok((Self { id, p2p, state }, libp2p_peer_id))
	}

	pub fn set_ipv4_enabled(&self, port: Option<u16>) -> Result<(), String> {
		// if let Some(port) = port {
		// } else {
		// 	//  let Some(addr) = self
		// 	// 	.state
		// 	// 	.read()
		// 	// 	.unwrap_or_else(PoisonError::into_inner)
		// 	// 	.ipv4_addr {

		// 	// 		self.p2p.unregister_listener_addr(self.id, addr);
		// 	// 	}
		// }

		// let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port.unwrap_or(0)));
		// self.p2p.register_listener_addr(self.id, addr)

		// todo!();

		Ok(())
	}

	pub fn set_ipv6_enabled(&self, port: Option<u16>) -> Result<(), String> {
		// todo!();

		Ok(())
	}

	pub fn shutdown(self) {
		self.p2p.unregister_hook(self.id.into());
	}
}

fn ok<T>(v: Result<T, Infallible>) -> T {
	match v {
		Ok(v) => v,
		Err(_) => unreachable!(),
	}
}

async fn start(
	p2p: Arc<P2P>,
	state: Arc<RwLock<State>>,
	mut swarm: Swarm<SpaceTime>,
	mut rx: Receiver<HookEvent>,
) {
	loop {
		tokio::select! {
			Ok(event) = rx.recv_async() => match event {
				HookEvent::Shutdown => break,
				_ => {},
			},
			event = swarm.select_next_some() => {
				todo!();
			}
		}
	}
}
