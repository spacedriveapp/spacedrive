use std::{
	collections::HashMap,
	net::{IpAddr, SocketAddr},
	pin::Pin,
	str::FromStr,
	sync::Arc,
	time::Duration,
};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::{sleep_until, Instant, Sleep};
use tracing::{debug, error, warn};

use crate::{AsyncFn, DiscoveredPeer, Event, Manager, Metadata, PeerId};

/// TODO
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

/// TODO
pub struct Mdns<TMetadata, TMetadataFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
{
	manager: Arc<Manager<TMetadata>>,
	fn_get_metadata: TMetadataFn,
	mdns_daemon: ServiceDaemon,
	mdns_service_receiver: flume::Receiver<ServiceEvent>,
	service_name: String,
	next_mdns_advertisement: Pin<Box<Sleep>>,
}

impl<TMetadata, TMetadataFn> Mdns<TMetadata, TMetadataFn>
where
	TMetadata: Metadata,
	TMetadataFn: AsyncFn<Output = TMetadata>,
{
	pub fn new(
		manager: Arc<Manager<TMetadata>>,
		application_name: &'static str,
		fn_get_metadata: TMetadataFn,
	) -> Result<Self, mdns_sd::Error>
	where
		TMetadataFn: AsyncFn<Output = TMetadata>,
	{
		let mdns_daemon = ServiceDaemon::new()?;
		let service_name = format!("_{}._udp.local.", application_name);
		let mdns_service_receiver = mdns_daemon.browse(&service_name)?;

		let this = Self {
			manager,
			fn_get_metadata,
			mdns_daemon,
			mdns_service_receiver,
			service_name,
			next_mdns_advertisement: Box::pin(sleep_until(
				Instant::now() + MDNS_READVERTISEMENT_INTERVAL,
			)),
		};
		this.advertise();
		Ok(this)
	}

	pub fn unregister_mdns(&self) -> mdns_sd::Result<mdns_sd::Receiver<mdns_sd::UnregisterStatus>> {
		self.mdns_daemon
			.unregister(&format!("{}.{}", self.manager.peer_id, self.service_name))
	}

	/// Do an mdns advertisement to the network
	pub fn advertise(&self) {
		// TODO: Instead of spawning maybe do this as part of the polling loop to avoid needing persitent reference to manager.
		let manager = self.manager.clone();
		let service_name = self.service_name.clone();
		// let fn_get_metadata = self.fn_get_metadata.clone();
		let mdns_daemon = self.mdns_daemon.clone();

		let metadata_fut = (self.fn_get_metadata)();

		tokio::spawn(async move {
			let metadata = metadata_fut.await.to_hashmap();
			let peer_id = manager.peer_id.0.to_base58();

			// This is in simple terms converts from `Vec<(ip, port)>` to `Vec<(Vec<Ip>, port)>`
			let mut services = HashMap::<u16, ServiceInfo>::new();
			for addr in manager.listen_addrs.read().await.iter() {
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
						&service_name,
						&peer_id,
						&format!("{}.", peer_id),
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
				debug!("advertising mdns service: {:?}", service);
				match mdns_daemon.register(service) {
					Ok(_) => {}
					Err(err) => warn!("error registering mdns service: {}", err),
				}
			}
		});
	}

	// TODO: if the channel's sender is dropped will this cause the `tokio::select` in the `manager.rs` to infinitely loop?
	pub async fn poll(&mut self) -> Option<Event<TMetadata>> {
		tokio::select! {
			_ = &mut self.next_mdns_advertisement => {
				self.advertise();
				self.next_mdns_advertisement = Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
			}
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
								if peer_id == self.manager.peer_id {
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
												self.manager.discovered.write().await;

											let peer = if let Some(peer) = discovered_peers.remove(&peer_id) {

												peer
											} else {
												DiscoveredPeer {
													manager: self.manager.clone(),
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
								if peer_id == self.manager.peer_id {
									return None;
								}

								{
									let mut discovered_peers =
										self.manager.discovered.write().await;
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
}
