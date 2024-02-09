use std::{
	convert::Infallible,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
	sync::{Arc, PoisonError, RwLock},
};

use flume::{bounded, Receiver, Sender};
use libp2p::{
	core::muxing::StreamMuxerBox,
	futures::StreamExt,
	swarm::dial_opts::{DialOpts, PeerCondition},
	Swarm, SwarmBuilder, Transport,
};
use stable_vec::StableVec;
use tokio::{
	net::TcpListener,
	sync::{mpsc, oneshot},
};
use tracing::warn;

use crate::{
	quic::libp2p::socketaddr_to_quic_multiaddr, ConnectionRequest, HookEvent, HookId, ListenerId,
	RemoteIdentity, UnicastStream, P2P,
};

use super::behaviour::SpaceTime;

/// [libp2p::PeerId] for debugging purposes only.
#[derive(Debug)]
pub struct Libp2pPeerId(libp2p::PeerId);

#[derive(Debug)]
enum InternalEvent {
	RegisterListener {
		id: ListenerId,
		ipv4: bool,
		addr: SocketAddr,
		result: oneshot::Sender<Result<(), String>>,
	},
	UnregisterListener {
		id: ListenerId,
		ipv4: bool,
		result: oneshot::Sender<Result<(), String>>,
	},
}

/// Transport using Quic to establish a connection between peers.
/// This uses `libp2p` internally.
#[derive(Debug)]
pub struct QuicTransport {
	id: ListenerId,
	p2p: Arc<P2P>,
	state: Arc<RwLock<State>>,
	internal_tx: Sender<InternalEvent>,
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
		let (internal_tx, internal_rx) = bounded(15);
		let (connect_tx, connect_rx) = mpsc::channel(15);
		let id = p2p.register_listener("libp2p-quic", tx, move |listener_id, peer, _addrs| {
			// TODO: I don't love this always being registered. Really it should only show up if the other device is online (do a ping-type thing)???
			peer.listener_available(listener_id, connect_tx.clone());
		});

		// let application_name = format!("/{}/spacetime/1.0.0", p2p.app_name());
		let swarm = ok(ok(SwarmBuilder::with_existing_identity(keypair)
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
		tokio::spawn(start(
			p2p.clone(),
			id,
			state.clone(),
			swarm,
			rx,
			internal_rx,
			connect_rx,
		));

		Ok((
			Self {
				id,
				p2p,
				state,
				internal_tx,
			},
			libp2p_peer_id,
		))
	}

	// `None` on the port means disabled. Use `0` for random port.
	pub async fn set_ipv4_enabled(&self, port: Option<u16>) -> Result<(), String> {
		self.setup_listener(
			port.map(|p| SocketAddr::from((Ipv4Addr::UNSPECIFIED, p))),
			true,
		)
		.await
	}

	pub async fn set_ipv6_enabled(&self, port: Option<u16>) -> Result<(), String> {
		self.setup_listener(
			port.map(|p| SocketAddr::from((Ipv6Addr::UNSPECIFIED, p))),
			false,
		)
		.await
	}

	// TODO: Proper error type
	async fn setup_listener(&self, addr: Option<SocketAddr>, ipv4: bool) -> Result<(), String> {
		let (tx, rx) = oneshot::channel();
		let event = if let Some(mut addr) = addr {
			if addr.port() == 0 {
				addr.set_port(
					TcpListener::bind(addr)
						.await
						.unwrap()
						.local_addr()
						.unwrap()
						.port(),
				);
			}

			InternalEvent::RegisterListener {
				id: self.id,
				ipv4,
				addr,
				result: tx,
			}
		} else {
			InternalEvent::UnregisterListener {
				id: self.id,
				ipv4,
				result: tx,
			}
		};

		let Ok(_) = self.internal_tx.send(event) else {
			return Err("internal channel closed".to_string());
		};
		rx.await
			.map_err(|_| "internal response channel closed".to_string())
			.and_then(|r| r)
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
	id: ListenerId,
	state: Arc<RwLock<State>>,
	mut swarm: Swarm<SpaceTime>,
	rx: Receiver<HookEvent>,
	internal_rx: Receiver<InternalEvent>,
	mut connect_rx: mpsc::Receiver<ConnectionRequest>,
) {
	let mut ipv4_listener = None;
	let mut ipv6_listener = None;

	loop {
		tokio::select! {
			Ok(event) = rx.recv_async() => match event {
				HookEvent::Shutdown => break,
				_ => {},
			},
			event = swarm.select_next_some() => match event {
				_ => {},
			},
			Ok(event) = internal_rx.recv_async() => match event {
				InternalEvent::RegisterListener { id, ipv4, addr, result } => {
					match swarm.listen_on(socketaddr_to_quic_multiaddr(&addr)) {
						Ok(libp2p_listener_id) => {
							let this = match ipv4 {
								true => &mut ipv4_listener,
								false => &mut ipv6_listener,
							};
							// TODO: Diff the `addr` & if it's changed actually update it
							if this.is_none() {
								*this =  Some((libp2p_listener_id, addr));
								p2p.register_listener_addr(id, addr);
							}

							let _ = result.send(Ok(()));
						},
						Err(e) => {
							let _ = result.send(Err(e.to_string()));
						},
					}
				},
				InternalEvent::UnregisterListener { id, ipv4, result } => {
					let this = match ipv4 {
						true => &mut ipv4_listener,
						false => &mut ipv6_listener,
					};
					if let Some((addr_id, addr)) = this.take() {
						if swarm.remove_listener(addr_id) {
							p2p.unregister_listener_addr(id, addr);
						}
					}
					let _ = result.send(Ok(()));
				},
			},
			Some(req) = connect_rx.recv() => {
				let opts = DialOpts::unknown_peer_id()
					// TODO: PR to libp2p to support multiple (their tech stack already supports it just not this builder)
					.address(socketaddr_to_quic_multiaddr(req.addrs.iter().next().unwrap()))
					// .address(req.addrs.iter().map(socketaddr_to_quic_multiaddr).collect())
					.build();
				let id = opts.connection_id();
				let Err(err) = swarm.dial(opts) else {
					swarm.behaviour_mut().state.establishing_outbound.lock().unwrap_or_else(PoisonError::into_inner).insert(id, req);
					return;
				};

				warn!(
					"error dialing peer '{}' with addresses '{:?}': {}",
					req.to, req.addrs, err
				);
				let _ = req.tx.send(Err(err.to_string()));
			}
		}
	}
}
