use std::{
	io,
	net::{IpAddr, Ipv4Addr, SocketAddr},
	sync::Arc,
	time::Duration,
};

use dashmap::{DashMap, DashSet};
use futures_util::StreamExt;
use if_watch::{IfEvent, IfWatcher};
use quinn::{Endpoint, Incoming, ServerConfig};
use rustls::{Certificate, PrivateKey};
use sd_tunnel_utils::{quic, PeerId};
use tokio::{select, time::sleep};

use crate::{
	handle_connection, GlobalDiscovery, Identity, NetworkManagerConfig, NetworkManagerError,
	P2PManager, MDNS,
};

/// TODO
pub struct NetworkManager<TP2PManager: P2PManager> {
	/// PeerId is the unique identifier of the current node.
	pub(crate) peer_id: PeerId,
	/// identity is the TLS identity of the current node.
	pub(crate) identity: (Certificate, PrivateKey),
	/// known_peers contains a list of all peers which are known to the network. These will be automatically connected if found.
	/// We store these so when making a request to the global discovery server we know who to lookup.
	pub(crate) known_peers: DashSet<PeerId>,
	/// TODO
	pub(crate) discovered_peers: DashMap<PeerId, ()>,
	/// TODO
	pub(crate) connected_peers: DashMap<PeerId, ()>,
	/// TODO
	pub(crate) lan_addrs: DashSet<Ipv4Addr>,
	/// TODO
	pub(crate) listen_addr: SocketAddr,
	/// manager is a type which implements P2PManager and is used so the NetworkManager can interact with the host application.
	pub(crate) manager: TP2PManager,
	/// endpoint is the QUIC endpoint that is used to send and receive network traffic between peers.
	pub(crate) endpoint: Endpoint,
}

impl<TP2PManager: P2PManager> NetworkManager<TP2PManager> {
	pub async fn new(
		identity: Identity,
		manager: TP2PManager,
		config: NetworkManagerConfig,
	) -> Result<Arc<Self>, NetworkManagerError> {
		if !TP2PManager::APPLICATION_NAME
			.chars()
			.all(char::is_alphanumeric)
		{
			return Err(NetworkManagerError::InvalidAppName);
		}

		let identity = identity.into_rustls();
		let (endpoint, mut incoming) = Endpoint::server(
			ServerConfig::with_crypto(Arc::new(quic::server_config(
				vec![identity.0.clone()],
				identity.1.clone(),
			)?)),
			format!("[::]:{}", config.listen_port.unwrap_or(0))
				.parse()
				.expect("unreachable error: invalid connection address. Please report if you encounter this error!"),
		)
		.map_err(NetworkManagerError::Server)?;

		let this = Arc::new(Self {
			peer_id: PeerId::from_cert(&identity.0),
			identity: identity,
			known_peers: config.known_peers.into_iter().collect(),
			discovered_peers: DashMap::new(),
			connected_peers: DashMap::new(),
			lan_addrs: DashSet::new(),
			listen_addr: endpoint.local_addr().map_err(NetworkManagerError::Server)?,
			manager,
			endpoint,
		});
		Self::event_loop(&this, incoming).await?;
		Ok(this)
	}

	/// returns the peer ID of the current node. These are unique identifier derived from the nodes public key.
	pub fn peer_id(&self) -> PeerId {
		self.peer_id.clone()
	}

	/// returns the address that the NetworkManager will listen on for incoming connections from other peers.
	pub fn listen_addr(&self) -> SocketAddr {
		self.listen_addr.clone()
	}

	// /// returns a list of the connected peers.
	// pub async fn connected_peers(&self) -> HashMap<PeerId, Peer> {
	// 	self.state.connected_peers.read().await.clone()
	// }

	// /// discovered_peers returns a list of the discovered peers.
	// pub fn discovered_peers(&self) -> HashMap<PeerId, PeerCandidate> {
	// 	self.discovery
	// 		.discovered_peers
	// 		.clone()
	// 		.into_iter()
	// 		.collect()
	// }

	async fn event_loop(
		nm: &Arc<Self>,
		mut quic_incoming: Incoming,
	) -> Result<(), NetworkManagerError> {
		let mut if_watcher = IfWatcher::new()
			.await
			.map_err(NetworkManagerError::IfWatch)?;
		let mdns = MDNS::init(nm)?;
		let global = GlobalDiscovery::init(nm)?;
		global.poll().await;

		for iface in if_watcher.iter() {
			Self::handle_ifwatch_event(nm, IfEvent::Up(iface.clone()));
		}

		Self::register(&mdns, &global).await; // TODO: Create a discovery stack type to hold them instead of passing them all individually

		let nm = nm.clone();
		tokio::spawn(async move {
			loop {
				// TODO: Deal with `Self::register`'s network calls blocking the main event loop
				select! {
					conn = quic_incoming.next() => match conn {
						Some(conn) => handle_connection(&nm, conn),
						None => break,
					},
					event = Pin::new(&mut if_watcher) => {
						match event {
							Ok(event) => {
								if Self::handle_ifwatch_event(&nm, event) {
									Self::register(&mdns, &global).await;
								}
							},
							Err(_) => break,
						}
					}
					_ = mdns.handle_mdns_event() => {}
					_ = sleep(Duration::from_secs(15 * 60 /* 15 Minutes */)) => {
						Self::register(&mdns, &global).await;
					}
					// TODO: Maybe use subscription system instead of polling or review this timeout!
					_ = sleep(Duration::from_secs(30 /* 30 Seconds */)) => {
						global.poll().await; // TODO: this does network calls and blocks. Is this ok?
					}
				};
			}
		});
		Ok(())
	}

	fn handle_ifwatch_event(nm: &Arc<Self>, event: IfEvent) -> bool {
		match event {
			IfEvent::Up(iface) => {
				let ip = match iface.addr() {
					IpAddr::V4(ip) if ip != Ipv4Addr::LOCALHOST => ip,
					_ => return false, // Currently IPv6 is not supported. Support will likely be added in the future.
				};
				nm.lan_addrs.insert(ip)
			}
			IfEvent::Down(iface) => {
				let ip = match iface.addr() {
					IpAddr::V4(ip) if ip != Ipv4Addr::LOCALHOST => ip,
					_ => return false, // Currently IPv6 is not supported. Support will likely be added in the future.
				};
				nm.lan_addrs.remove(&ip).is_some()
			}
		}
	}

	pub(crate) async fn register(mdns: &MDNS<TP2PManager>, global: &GlobalDiscovery<TP2PManager>) {
		mdns.register().await;
		global.register().await;
	}

	fn shutdown() {
		// TODO: Trigger this function
		// TODO: Deannounce MDNS + Global Discovery
	}
}
