use std::{
	collections::{HashMap, HashSet},
	marker::PhantomData,
	net::{IpAddr, SocketAddr},
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use futures::StreamExt;
use libp2p::{
	core::muxing::StreamMuxerBox,
	quic,
	swarm::{
		dial_opts::{DialOpts, PeerCondition},
		SwarmEvent,
	},
	Multiaddr, Swarm, Transport,
};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use thiserror::Error;
use tokio::{
	sync::{mpsc, oneshot, RwLock},
	time::sleep,
};
use tracing::{debug, error, warn};

use crate::{
	event::Event,
	spacetime::{SpaceTime, SpaceTimeMessage, SpaceTimeResponseChan},
	utils::{quic_multiaddr_to_socketaddr, socketaddr_to_quic_multiaddr, AsyncFn, AsyncFn2},
	Connection, DiscoveredPeer, Keypair, ManagerRef, Metadata, PeerId,
};

/// TODO
pub struct Manager<TMetadata, TMetadataFn, TEventFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, Event<TMetadata>, Output = ()>,
{
	state: Arc<ManagerRef<TMetadata>>,

	fn_get_metadata: TMetadataFn,
	fn_on_event: Arc<TEventFn>,
	mdns_daemon: ServiceDaemon,

	phantom: PhantomData<TMetadata>,
}

impl<TMetadata, TMetadataFn, TEventFn> Manager<TMetadata, TMetadataFn, TEventFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
	TEventFn: AsyncFn2<Arc<ManagerRef<TMetadata>>, Event<TMetadata>, Output = ()>,
{
	/// create a new P2P manager. Please do your best to make the callback closures as fast as possible because they will slow the P2P event loop!
	pub async fn new<TConnFn>(
		application_name: &'static str,
		keypair: &Keypair,
		fn_get_metadata: TMetadataFn,
		fn_on_event: TEventFn,
		fn_on_connect: TConnFn,
	) -> Result<Arc<Self>, ManagerError>
	where
		TConnFn: AsyncFn2<Connection<TMetadata>, Vec<u8>, Output = Result<Vec<u8>, ()>>,
	{
		application_name
			.chars()
			.all(char::is_alphanumeric)
			.then_some(())
			.ok_or(ManagerError::InvalidAppName)?;

		let mdns_daemon = ServiceDaemon::new()?;
		let service_name = format!("_{}._udp.local.", application_name);

		let fn_on_connect = Arc::new(fn_on_connect); // TODO: Avoid arcing it?
		let fn_on_event = Arc::new(fn_on_event); // TODO: Avoid arcing it?

		let (internal_tx, mut internal_rx) = mpsc::channel(250);
		let mut mdns_service = mdns_daemon.browse(&service_name).map(|r| r.into_stream())?;
		let this = Arc::new(Self {
			state: Arc::new(ManagerRef {
				service_name,
				peer_id: PeerId(keypair.public().to_peer_id()),
				internal_tx,
				listen_addrs: RwLock::new(Default::default()),
				discovered_peers: RwLock::new(Default::default()),
			}),
			fn_get_metadata,
			fn_on_event: fn_on_event.clone(),
			mdns_daemon,
			phantom: PhantomData,
		});

		let mut swarm = Swarm::with_tokio_executor(
			quic::GenTransport::<quic::tokio::Provider>::new(quic::Config::new(keypair.inner()))
				.map(|(p, c), _| (p, StreamMuxerBox::new(c)))
				.boxed(),
			SpaceTime::new(fn_on_connect, this.state.clone(), fn_on_event),
			keypair.public().to_peer_id(),
		);
		{
			let listener_id = swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse().expect("Error passing libp2p multiaddr. This value is hardcoded so this should be impossible."))
            .unwrap();
			debug!("created ipv4 listener with id '{:?}'", listener_id);
		}
		{
			let listener_id = swarm
        .listen_on("/ip6/::/udp/0/quic-v1".parse().expect("Error passing libp2p multiaddr. This value is hardcoded so this should be impossible."))
        .unwrap();
			debug!("created ipv4 listener with id '{:?}'", listener_id);
		}

		// TODO: Drop on manager drop
		tokio::spawn({
			let this = this.clone();
			let is_advertisement_queued = AtomicBool::new(false);
			// let mut active_requests = HashMap::new(); // Active Spacetime requests // TODO: Move this into SpaceTime

			async move {
				loop {
					// TODO: Look into using `add_address` and `remove_address` on the Swarm. This should be hooked up with mDNS so the outbound dialing on `.send` can establish the connection.
					// TODO: Don't create a connection with the peer until it's required? -> Eg. this event loop shouldn't create a connection!

					tokio::select! {
						event = internal_rx.recv() => {
							// TODO: Correctly handle `unwrap`
							match event.unwrap() {
								ManagerEvent::Dial(peer_id, addresses) => {
									debug!("dialing peer '{}' at addresses '{:?}'", peer_id, addresses);
									match swarm.dial(DialOpts::peer_id(peer_id.0)
										.condition(PeerCondition::Disconnected)
										.addresses(addresses.iter().map(|addr| socketaddr_to_quic_multiaddr(addr)).collect())
										.extend_addresses_through_behaviour()
										.build()) {
											Ok(_) => {},
											Err(err) => warn!("error dialing peer '{}' with addresses '{:?}': {}", peer_id, addresses, err),
									}
								},
								ManagerEvent::SendRequest(peer_id, data, resp) => {
									swarm.behaviour_mut().send(&peer_id, data, resp);
								},
								ManagerEvent::BroadcastRequest(data, resp) => {
									// TODO: Handle `resp` for collating results
									swarm.behaviour_mut().broadcast(data);
								},
							}
						}
						event = swarm.select_next_some() => {
							match event {
								SwarmEvent::Behaviour(event) => {}, // TODO: event.handle(&this.state, this.fn_on_connect.clone()).await,
								SwarmEvent::ConnectionEstablished { .. } => {},
								SwarmEvent::ConnectionClosed { .. } => {},
								// TODO: Move rest of these
								SwarmEvent::IncomingConnection { local_addr, .. } => debug!("incoming connection from '{}'", local_addr),
								SwarmEvent::IncomingConnectionError { local_addr, error, .. } => warn!("handshake error with incoming connection from '{}': {}", local_addr, error),
								SwarmEvent::OutgoingConnectionError { peer_id, error } => warn!("error establishing connection with '{:?}': {}", peer_id, error),
								SwarmEvent::BannedPeer { peer_id, .. } => warn!("banned peer '{}' attempted to connection and was rejected", peer_id),
								SwarmEvent::NewListenAddr{ address, .. } => {
									match quic_multiaddr_to_socketaddr(address) {
										Ok(addr) => {
											debug!("listen address added: {}", addr);
											this.state.listen_addrs.write().await.insert(addr);
											if !is_advertisement_queued.load(Ordering::Relaxed) {
												is_advertisement_queued.store(true, Ordering::Relaxed);
												tokio::spawn(this.clone().advertise());
											}
											(this.fn_on_event)(this.state.clone(), Event::AddListenAddr(addr)).await;
										},
										Err(err) => {
											warn!("error passing listen address: {}", err);
											continue;
										}
									}
								},
								SwarmEvent::ExpiredListenAddr { address, .. } => {
									match this.unregister_addr(address, &is_advertisement_queued).await {
										Ok(_) => {},
										Err(err) => {
											warn!("error passing listen address: {}", err);
											continue;
										}
									}
								}
								SwarmEvent::ListenerClosed { listener_id, addresses, reason } => {
									debug!("listener '{:?}' was closed due to: {:?}", listener_id, reason);
									for address in addresses {
										match this.unregister_addr(address, &is_advertisement_queued).await {
											Ok(_) => {},
											Err(err) => {
												warn!("error passing listen address: {}", err);
												continue;
											}
										}
									}
								}
								SwarmEvent::ListenerError { listener_id, error } => warn!("listener '{:?}' reported a non-fatal error: {}", listener_id, error),
								SwarmEvent::Dialing(_peer_id) => {},
							}
						}
						event = mdns_service.next() => {
							// TODO: Correctly handle `unwrap`
							match event.unwrap() {
								ServiceEvent::SearchStarted(_) => {}
								ServiceEvent::ServiceFound(_, _) => {}
								ServiceEvent::ServiceResolved(info) => {
									let raw_peer_id = info
										.get_fullname()
										.replace(&format!(".{}", this.state.service_name), "");

									match PeerId::from_str(&raw_peer_id) {
										Ok(peer_id) => {
											// Prevent discovery of the current peer.
											if peer_id == this.state.peer_id  { continue }

											match TMetadata::from_hashmap(info.get_properties()) {
												Ok(metadata) => {
													let peer = {
														let mut discovered_peers = this.state.discovered_peers.write().await;

														let peer = if let Some(peer) = discovered_peers.remove(&peer_id) {
															// peer.addresses
															peer
														} else {
															DiscoveredPeer { id: peer_id, metadata, addresses: info.get_addresses().iter().map(|addr| SocketAddr::new(IpAddr::V4(addr.clone()), info.get_port())).collect() }
														};

														discovered_peers.insert(peer_id, peer.clone());
														peer
													};
													(this.fn_on_event)(this.state.clone(), Event::PeerDiscovered(peer)).await;
												}
												Err(err) => error!("error parsing metadata for peer '{}': {}", raw_peer_id, err),
											}
										}
										Err(_) => warn!(
											"resolved peer advertising itself with an invalid peer_id '{}'",
											raw_peer_id
										),
									}
								}
								ServiceEvent::ServiceRemoved(_, fullname) => {
									let raw_peer_id = fullname.replace(&format!(".{}", this.state.service_name), "");

									match PeerId::from_str(&raw_peer_id) {
										Ok(peer_id) => {
											// Prevent discovery of the current peer.
											if peer_id == this.state.peer_id  { continue }

											{
												let mut discovered_peers = this.state.discovered_peers.write().await;
												let peer = discovered_peers.remove(&peer_id);

												(this.fn_on_event)(this.state.clone(), Event::PeerExpired { id: peer_id, metadata: peer.map(|p| p.metadata) }, ).await;
											}
										}
										Err(_) => warn!(
											"resolved peer de-advertising itself with an invalid peer_id '{}'",
											raw_peer_id
										),
									}
								}
								ServiceEvent::SearchStopped(_) => {}
							}
						}
						_ = sleep(Duration::from_secs(120)) => {
							tokio::spawn(this.clone().advertise());
						}
						// TODO: Broadcast MDNS shutdown on app shutdown -> TODO: Add method for manually doing this on the manager
					}
				}
			}
		});

		Ok(this)
	}

	async fn unregister_addr(
		self: &Arc<Self>,
		address: Multiaddr,
		is_advertisement_queued: &AtomicBool,
	) -> Result<(), String> {
		match quic_multiaddr_to_socketaddr(address) {
			Ok(addr) => {
				debug!("listen address removed: {}", addr);
				self.state.listen_addrs.write().await.remove(&addr);
				let _ = self.unregister_mdns();
				if !is_advertisement_queued.load(Ordering::Relaxed) {
					is_advertisement_queued.store(true, Ordering::Relaxed);
					tokio::spawn(self.clone().advertise());
				}
				(self.fn_on_event)(self.state.clone(), Event::RemoveListenAddr(addr)).await;
				Ok(())
			}
			Err(err) => Err(err),
		}
	}

	fn unregister_mdns(
		self: &Arc<Self>,
	) -> mdns_sd::Result<mdns_sd::Receiver<mdns_sd::UnregisterStatus>> {
		self.mdns_daemon.unregister(&format!(
			"{}.{}",
			self.state.peer_id, self.state.service_name
		))
	}

	/// Do an mdns advertisement to the network
	async fn advertise(self: Arc<Self>) {
		let peer_id = self.state.peer_id.0.to_base58();

		// This is in simple terms converts from `Vec<(ip, port)>` to `Vec<(Vev<Ip>, port)>`
		let mut services = HashMap::<u16, ServiceInfo>::new();
		for addr in self.state.listen_addrs.read().await.iter() {
			let addr = match addr {
				SocketAddr::V4(addr) => addr,
				// TODO: Our mdns library doesn't support Ipv6. This code has the infra to support it so once this issue is fixed upstream we can just flip it on.
				// Refer to issue: https://github.com/keepsimple1/mdns-sd/issues/61
				SocketAddr::V6(_) => continue,
			};

			if let Some(mut service) = services.remove(&addr.port()) {
				service.insert_ipv4addr(*addr.ip());
				services.insert(addr.port(), service);
			} else {
				let service = match ServiceInfo::new(
					&self.state.service_name,
					&peer_id,
					&format!("{}.", peer_id),
					*addr.ip(),
					addr.port(),
					Some((self.fn_get_metadata)().await.to_hashmap()), // TODO: Prevent the user defining a value that overflows a DNS record
				) {
					Ok(service) => service,
					Err(err) => {
						warn!("error creating mdns service info: {}", err);
						continue;
					}
				};
				services.insert(addr.port(), service);
			}
		}

		for (_, service) in services.into_iter() {
			debug!("advertising mdns service: {:?}", service);
			match self.mdns_daemon.register(service) {
				Ok(_) => {}
				Err(err) => warn!("error registering mdns service: {}", err),
			}
		}
	}

	pub fn peer_id(&self) -> PeerId {
		self.state.peer_id
	}

	pub async fn listen_addrs(&self) -> HashSet<SocketAddr> {
		self.state.listen_addrs.read().await.clone()
	}

	// TODO: Proper error type
	pub async fn send(&self, peer_id: PeerId, data: Vec<u8>) -> Result<Vec<u8>, ()> {
		// TODO: With this system you can send to any random peer id. Can I reduce that by requiring `.connect(peer_id).unwrap().send(data)` or something like that.

		let (tx, rx) = oneshot::channel();
		self.state
			.internal_tx
			.send(ManagerEvent::SendRequest(
				peer_id,
				SpaceTimeMessage::Application(data),
				Some(tx),
			))
			.await
			.map_err(|_| ())?;

		match rx.await {
			Ok(Ok(SpaceTimeMessage::Application(data))) => Ok(data),
			// Ok(Err(OutboundFailure::ConnectionClosed)) => {
			// 	// TODO: Ensure we remove it from the connected peers list if we missed it somewhere else
			// 	Err(())
			// }
			// TODO: Error handling
			err => {
				error!("TODO: Broadcast error: {:?}", err);
				Err(())
			}
		}
	}

	// TODO: Error's should be collected and message should attempt to be send to everyone and not fail early
	// TODO: Return channel which can be awaited to get report of broadcast -> How many sent vs dropped
	// TODO: Allow targeting a set of peer_ids with the broadcast
	pub async fn broadcast(self: &Arc<Self>, data: impl Into<Vec<u8>>) -> Result<(), ()> {
		self.state
			.internal_tx
			.send(ManagerEvent::BroadcastRequest(
				SpaceTimeMessage::Application(data.into()),
				None,
			))
			.await
			.map_err(|_| ())
	}

	pub async fn get_discovered_peers(&self) -> Vec<DiscoveredPeer<TMetadata>> {
		self.state
			.discovered_peers
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}
}

#[derive(Error, Debug)]
pub enum ManagerError {
	#[error(
		"the application name you application provided is invalid. Ensure it is alphanumeric!"
	)]
	InvalidAppName,
	#[error("error with mdns discovery: {0}")]
	Mdns(#[from] mdns_sd::Error),
}

pub(crate) enum ManagerEvent {
	Dial(PeerId, Vec<SocketAddr>),
	SendRequest(PeerId, SpaceTimeMessage, Option<SpaceTimeResponseChan>),
	BroadcastRequest(SpaceTimeMessage, Option<SpaceTimeResponseChan>),
}
