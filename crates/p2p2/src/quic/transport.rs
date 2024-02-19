use std::{
	collections::HashMap,
	convert::Infallible,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
	sync::{Arc, PoisonError, RwLock},
	time::Duration,
};

use flume::{bounded, Receiver, Sender};
use libp2p::{
	core::muxing::StreamMuxerBox,
	futures::{AsyncReadExt, AsyncWriteExt, StreamExt},
	StreamProtocol, Swarm, SwarmBuilder, Transport,
};
use libp2p_stream::Behaviour;
use tokio::{
	net::TcpListener,
	sync::{mpsc, oneshot},
};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, warn};

use crate::{
	identity::REMOTE_IDENTITY_LEN,
	quic::utils::{
		identity_to_libp2p_keypair, remote_identity_to_libp2p_peerid, socketaddr_to_quic_multiaddr,
	},
	ConnectionRequest, HookEvent, ListenerId, RemoteIdentity, UnicastStream, P2P,
};

const PROTOCOL: StreamProtocol = StreamProtocol::new("/sdp2p/1");

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
		let keypair = identity_to_libp2p_keypair(p2p.identity());
		let libp2p_peer_id = Libp2pPeerId(keypair.public().to_peer_id());

		let (tx, rx) = bounded(15);
		let (internal_tx, internal_rx) = bounded(15);
		let (connect_tx, connect_rx) = mpsc::channel(15);
		let id = p2p.register_listener("libp2p-quic", tx, move |listener_id, peer, _addrs| {
			// TODO: I don't love this always being registered. Really it should only show up if the other device is online (do a ping-type thing)???
			peer.listener_available(listener_id, connect_tx.clone());
		});

		let swarm = ok(ok(SwarmBuilder::with_existing_identity(keypair)
			.with_tokio()
			.with_other_transport(|keypair| {
				libp2p_quic::GenTransport::<libp2p_quic::tokio::Provider>::new(
					libp2p_quic::Config::new(keypair),
				)
				.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
				.boxed()
			}))
		.with_behaviour(|_| Behaviour::new()))
		.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
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
	state: Arc<RwLock<State>>, // TODO: Removing this or using it???
	mut swarm: Swarm<Behaviour>,
	rx: Receiver<HookEvent>,
	internal_rx: Receiver<InternalEvent>,
	mut connect_rx: mpsc::Receiver<ConnectionRequest>,
) {
	let mut ipv4_listener = None;
	let mut ipv6_listener = None;

	let mut control = swarm.behaviour().new_control();
	let mut incoming = control.accept(PROTOCOL).unwrap(); // TODO: Error handling

	loop {
		tokio::select! {
			Ok(event) = rx.recv_async() => match event {
				HookEvent::Shutdown => break,
				_ => {},
			},
			Some((peer_id, mut stream)) = incoming.next() => {
				let p2p = p2p.clone();
				tokio::spawn(async move {
					println!("GOT STREAM FOR {:?}", peer_id); // TODO

					let mut actual = [0; REMOTE_IDENTITY_LEN];
					stream.read_exact(&mut actual).await.unwrap(); // TODO: Error handling
					let identity = RemoteIdentity::from_bytes(&actual).unwrap();

					// We need to go `PeerId -> RemoteIdentity` but as `PeerId` is a hash that's impossible.
					// So to make this work the connection initiator will send their remote identity.
					// It is however untrusted as they could send anything, so we convert it to a PeerId and check it matches the PeerId for this connection.
					// If it matches, we are certain they own the private key as libp2p takes care of ensuring the PeerId is trusted.
					let remote_identity_peer_id = remote_identity_to_libp2p_peerid(&identity);
					if peer_id != remote_identity_peer_id {
						panic!("no please don't hack me, hacker man"); // TODO: Error handling
					}

					// TODO: Sync metadata
					let metadata = HashMap::new();

					let stream = UnicastStream::new(identity, stream.compat());
					// debug!(
					// 	"stream({id}): established stream with '{}'",
					// 	stream.remote_identity()
					// );


					let (shutdown_tx, shutdown_rx) = oneshot::channel();
					let peer = p2p.connected_to(
						id,
						metadata,
						stream,
						shutdown_tx,
					);
					// println!("INITIALISED PEER: {:?}", peer); // TODO

					// TODO: Handle `shutdown_rx`
				});
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
				let mut control = control.clone();
				let self_remote_identity = p2p.identity().to_remote_identity();
				tokio::spawn(async move {
					let peer_id = remote_identity_to_libp2p_peerid(&req.to);
					let mut stream = control.open_stream_with_addrs(
						peer_id,
						PROTOCOL,
						req.addrs.iter()
							// TODO: Remove this `filter`
							.filter(|a| a.is_ipv4())
							.map(socketaddr_to_quic_multiaddr)
							.collect()
					).await.unwrap();  // TODO: Error handling send back to caller though channel

					stream.write_all(&self_remote_identity.get_bytes()).await.unwrap(); // TODO: Error handling

					// TODO: Sync metadata

					// TODO: Convert into log
					println!("ESTABLISHED STREAM: {:?}", stream); // TODO

					let _ = req.tx.send(Ok(UnicastStream::new(req.to, stream.compat())));
				});
			}
		}
	}
}
