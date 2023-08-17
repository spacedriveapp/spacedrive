use std::{
	collections::{HashMap, HashSet},
	net::{IpAddr, SocketAddr},
	pin::Pin,
	str::FromStr,
	sync::{Arc, PoisonError, RwLock},
	time::Duration,
};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::{
	sync::mpsc,
	time::{sleep_until, Instant, Sleep},
};
use tracing::{error, info, warn};

use crate::{Component, DiscoveredPeer, Event, Metadata, MetadataManager, PeerId};

/// TODO
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

// TODO: Merge onto `Mdns`?
// TODO: Removing locking?
/// TODO
#[derive(Debug)]
pub struct MdnsState<TMetadata: Metadata> {
	pub discovered: RwLock<HashMap<PeerId, DiscoveredPeer<TMetadata>>>,
	// TODO: Remove this
	pub listen_addrs: RwLock<HashSet<SocketAddr>>,
}

/// TODO
pub struct Mdns<TMetadata>
where
	TMetadata: Metadata,
{
	// used to ignore events from our own mdns advertisement
	peer_id: PeerId,
	metadata_manager: Arc<MetadataManager<TMetadata>>,
	mdns_daemon: ServiceDaemon,
	mdns_service_receiver: flume::Receiver<ServiceEvent>,
	service_name: String,
	next_mdns_advertisement: Pin<Box<Sleep>>,
	trigger_advertisement: mpsc::UnboundedReceiver<()>,
	pub(crate) state: Arc<MdnsState<TMetadata>>,
}

impl<TMetadata> Mdns<TMetadata>
where
	TMetadata: Metadata,
{
	pub fn new(
		application_name: &'static str,
		peer_id: PeerId,
		metadata_manager: Arc<MetadataManager<TMetadata>>,
	) -> Result<Self, mdns_sd::Error> {
		let mdns_daemon = ServiceDaemon::new()?;
		let service_name = format!("_{}._udp.local.", application_name);
		let mdns_service_receiver = mdns_daemon.browse(&service_name)?;
		let (advertise_tx, advertise_rx) = mpsc::unbounded_channel();

		// TODO: problematic when this is now a public API
		// metadata_manager.set_tx(advertise_tx).await;

		let state = Arc::new(MdnsState {
			discovered: Default::default(),
			listen_addrs: Default::default(),
		});

		Ok(Self {
			peer_id,
			metadata_manager,
			mdns_daemon,
			mdns_service_receiver,
			service_name,
			next_mdns_advertisement: Box::pin(sleep_until(Instant::now())), // Trigger an advertisement immediately
			trigger_advertisement: advertise_rx,
			state,
		})
	}

	pub fn unregister_mdns(&self) -> mdns_sd::Result<mdns_sd::Receiver<mdns_sd::UnregisterStatus>> {
		self.mdns_daemon
			.unregister(&format!("{}.{}", self.peer_id, self.service_name))
	}

	/// Do an mdns advertisement to the network.
	fn advertise(&mut self) {
		let metadata = self.metadata_manager.get().to_hashmap();

		// This is in simple terms converts from `Vec<(ip, port)>` to `Vec<(Vec<Ip>, port)>`
		let mut services = HashMap::<u16, ServiceInfo>::new();
		for addr in self
			.state
			.listen_addrs
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
		{
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
					&self.service_name,
					&self.peer_id.to_string(),
					&format!("{}.", self.peer_id),
					*addr.ip(),
					addr.port(),
					Some(metadata.clone()), // TODO: Prevent the user defining a value that overflows a DNS record
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
			info!("advertising mdns service: {:?}", service);
			match self.mdns_daemon.register(service) {
				Ok(_) => {}
				Err(err) => warn!("error registering mdns service: {}", err),
			}
		}

		self.next_mdns_advertisement =
			Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
	}

	// TODO: if the channel's sender is dropped will this cause the `tokio::select` in the `manager.rs` to infinitely loop?
	pub async fn poll(&mut self) -> Option<Event<TMetadata>> {
		tokio::select! {
			_ = &mut self.next_mdns_advertisement => self.advertise(),
			_ = self.trigger_advertisement.recv() => self.advertise(),
			event = self.mdns_service_receiver.recv_async() => {
				let event = event.unwrap(); // TODO: Error handling
				match event {
					ServiceEvent::SearchStarted(_) => {}
					ServiceEvent::ServiceFound(_, _) => {}
					ServiceEvent::ServiceResolved(info) => {
						let raw_peer_id = info
							.get_fullname()
							.replace(&format!(".{}", self.service_name), "");

						match PeerId::from_str(&raw_peer_id) {
							Ok(peer_id) => {
								// Prevent discovery of the current peer.
								if peer_id == self.peer_id {
									return None;
								}

								match TMetadata::from_hashmap(
									&info
										.get_properties()
										.iter()
										.map(|v| (v.key().to_owned(), v.val().to_owned()))
										.collect(),
								) {
									Ok(metadata) => {
										let peer = {
											let mut discovered_peers =
												self.state.discovered.write().unwrap_or_else(PoisonError::into_inner);

											let peer = if let Some(peer) = discovered_peers.remove(&peer_id) {

												peer
											} else {
												DiscoveredPeer {
													// TODO: Probs don't hold it
													manager: todo!(), // manager.clone(),
													peer_id,
													metadata,
													addresses: info
														.get_addresses()
														.iter()
														.map(|addr| {
															SocketAddr::new(
																IpAddr::V4(*addr),
																info.get_port(),
															)
														})
														.collect(),
												}
											};

											discovered_peers.insert(peer_id, peer.clone());
											peer
										};
										return Some(Event::PeerDiscovered(peer));
									}
									Err(err) => {
										error!("error parsing metadata for peer '{}': {}", raw_peer_id, err)
									}
								}
							}
							Err(_) => warn!(
								"resolved peer advertising itself with an invalid peer_id '{}'",
								raw_peer_id
							),
						}
					}
					ServiceEvent::ServiceRemoved(_, fullname) => {
						let raw_peer_id = fullname.replace(&format!(".{}", self.service_name), "");

						match PeerId::from_str(&raw_peer_id) {
							Ok(peer_id) => {
								// Prevent discovery of the current peer.
								if peer_id == self.peer_id {
									return None;
								}

								{
									let mut discovered_peers =
										self.state.discovered.write().unwrap_or_else(PoisonError::into_inner);
									let peer = discovered_peers.remove(&peer_id);

									return Some(Event::PeerExpired {
										id: peer_id,
										metadata: peer.map(|p| p.metadata),
									});
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
		};

		None
	}

	pub fn queue_advertisement(&mut self) {
		// If the next mdns advertisement is more than 250ms away, then we should queue one closer to now.
		// This acts as a debounce for advertisements when many addresses are discovered close to each other (Eg. at startup)
		if self.next_mdns_advertisement.deadline() > (Instant::now() + Duration::from_millis(250)) {
			self.next_mdns_advertisement =
				Box::pin(sleep_until(Instant::now() + Duration::from_millis(200)));
		}
	}

	pub fn shutdown(&self) {
		match self
			.mdns_daemon
			.unregister(&format!("{}.{}", self.peer_id, self.service_name))
			.map(|chan| chan.recv())
		{
			Ok(Ok(_)) => {}
			Ok(Err(err)) => {
				warn!(
					"shutdown error recieving shutdown status from mdns service: {}",
					err
				);
			}
			Err(err) => {
				warn!("shutdown error unregistering mdns service: {}", err);
			}
		}

		self.mdns_daemon.shutdown().unwrap_or_else(|err| {
			error!("shutdown error shutting down mdns daemon: {}", err);
		});
	}
}
