use std::{net::Ipv4Addr, sync::Arc};

use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent, ServiceInfo};
use sd_tunnel_utils::PeerId;
use tracing::warn;

use crate::{NetworkManager, NetworkManagerError, P2PManager, PeerCandidate, PeerMetadata};

/// MDNS is the discovery system used for over local networks. It makes use of Multicast DNS (mDNS) to discover peers.
/// It should also conforms to the mDNS SD specification.
pub(crate) struct Mdns<TP2PManager: P2PManager> {
	nm: Arc<NetworkManager<TP2PManager>>,
	mdns: ServiceDaemon,
	browser: Receiver<ServiceEvent>,
	service_type: String,
}

impl<TP2PManager: P2PManager> Mdns<TP2PManager> {
	pub fn init(nm: &Arc<NetworkManager<TP2PManager>>) -> Result<Self, NetworkManagerError> {
		tracing::debug!("Starting mdns discovery service");
		let mdns = ServiceDaemon::new()?;
		let service_type = format!("_{}._udp.local.", TP2PManager::APPLICATION_NAME);

		Ok(Self {
			nm: nm.clone(),
			browser: mdns.browse(&service_type)?,
			mdns,
			service_type,
		})
	}

	pub async fn handle_mdns_event(&self) {
		match self.browser.recv_async().await {
			Ok(event) => {
				tracing::debug!("Handling incoming mdns event: {:?}", event);
				match event {
					ServiceEvent::SearchStarted(_) => {}
					ServiceEvent::ServiceFound(_, _) => {}
					ServiceEvent::ServiceResolved(info) => {
						let raw_peer_id = info
							.get_fullname()
							.replace(&format!(".{}", self.service_type), "");
						match PeerId::from_string(raw_peer_id.clone()) {
							Ok(peer_id) => {
								// Prevent discovery of the current peer.
								if peer_id == self.nm.peer_id {
									return;
								}

								let peer = PeerCandidate {
									id: peer_id.clone(),
									metadata: PeerMetadata::from_hashmap(
										&peer_id,
										info.get_properties(),
									),
									addresses: info.get_addresses().iter().copied().collect(),
									port: info.get_port(),
								};

								self.nm.add_discovered_peer(peer);
							}
							Err(_) => {
								warn!(
									"resolved peer advertising itself with an invalid peer_id '{}'",
									raw_peer_id
								);
							}
						}
					}
					ServiceEvent::ServiceRemoved(_, fullname) => {
						let raw_peer_id = fullname.replace(&format!(".{}", self.service_type), "");
						match PeerId::from_string(raw_peer_id.clone()) {
							Ok(peer_id) => {
								// Prevent discovery of the current peer.
								if peer_id == self.nm.peer_id {
									return;
								}

								self.nm.remove_discovered_peer(peer_id);
							}
							Err(_) => {
								warn!(
									"resolved peer advertising itself with an invalid peer_id '{}'",
									raw_peer_id
								);
							}
						}
					}
					ServiceEvent::SearchStopped(_) => {}
				}
			}
			Err(err) => {
				tracing::warn!(
					"Error receiving MDNS event as the ServiceDaemon has been shut down: {:?}",
					err
				);
				tracing::info!("Error receiving MDNS event as the ServiceDaemon has been shut down. Local discovery has been disabled, please restart your app to re-enable local discovery!");
			}
		}
	}

	pub async fn register(&self) {
		let peer_id_str = &self.nm.peer_id.to_string();
		let service_info = ServiceInfo::new(
			&self.service_type,
			peer_id_str,
			&format!("{peer_id_str}."),
			&(self
				.nm
				.lan_addrs
				.iter()
				.map(|v| *v)
				.collect::<Vec<Ipv4Addr>>())[..],
			self.nm.listen_addr.port(),
			Some(self.nm.manager.get_metadata().to_hashmap()),
		);
		tracing::debug!("Registering mdns service entry: {:?}", service_info);

		match service_info {
			Ok(service_info) => match self.mdns.register(service_info) {
				Ok(_) => {}
				Err(err) => {
					warn!("failed to register mdns service: {}", err);
				}
			},
			Err(err) => {
				warn!("failed to register mdns service: {}", err);
			}
		}
	}

	/// shutdown shuts down the MDNS service. This will advertise the current peer as unavailable to the rest of the network.
	pub(crate) fn shutdown(&self) {
		tracing::debug!("Shutting down mdns discovery service");

		// The panics caused by `.expect` are acceptable here because they are run during shutdown where nothing can be done if they were to fail.
		self.mdns
			.unregister(&format!("{}.{}", self.nm.peer_id, self.service_type))
			.expect("Error unregistering the mDNS service")
			.recv()
			.expect("Error unregistering the mDNS service");

		self.mdns
			.shutdown()
			.expect("Error shutting down mDNS service");
	}
}
