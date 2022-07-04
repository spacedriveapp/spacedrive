use std::sync::Arc;

use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent, ServiceInfo};
use sd_tunnel_utils::PeerId;

use crate::{NetworkManager, NetworkManagerError, P2PManager, PeerCandidate, PeerMetadata};

/// TODO
pub(crate) struct MDNS<TP2PManager: P2PManager> {
	nm: Arc<NetworkManager<TP2PManager>>,
	mdns: ServiceDaemon,
	browser: Receiver<ServiceEvent>,
	service_type: String,
}

impl<TP2PManager: P2PManager> MDNS<TP2PManager> {
	pub fn init(nm: &Arc<NetworkManager<TP2PManager>>) -> Result<Self, NetworkManagerError> {
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
				match event {
					ServiceEvent::SearchStarted(_) => {}
					ServiceEvent::ServiceFound(_, _) => {}
					ServiceEvent::ServiceResolved(info) => {
						let raw_peer_id = info
							.get_fullname()
							.replace(&format!(".{}", self.service_type), "");
						match PeerId::from_str(raw_peer_id.clone()) {
							Ok(peer_id) => {
								// Prevent discovery of the current node.
								if peer_id == self.nm.peer_id {
									return;
								}

								let peer = PeerCandidate {
									id: peer_id.clone(),
									metadata: PeerMetadata::from_hashmap(
										&peer_id,
										info.get_properties(),
									),
									addresses: info
										.get_addresses()
										.iter()
										.map(|addr| addr.clone())
										.collect(),
									port: info.get_port(),
								};

								self.nm.add_discovered_peer(peer);
							}
							Err(_) => {
								println!("p2p warning: resolved node advertising itself with an invalid peer_id '{}'", raw_peer_id);
							}
						}
					}
					ServiceEvent::ServiceRemoved(_, fullname) => {
						let raw_peer_id = fullname.replace(&format!(".{}", self.service_type), "");
						match PeerId::from_str(raw_peer_id.clone()) {
							Ok(peer_id) => {
								// Prevent discovery of the current node.
								if peer_id == self.nm.peer_id {
									return;
								}

								self.nm.remove_discovered_peer(peer_id);
							}
							Err(_) => {
								println!("p2p warning: resolved node advertising itself with an invalid peer_id '{}'", raw_peer_id);
							}
						}
					}
					ServiceEvent::SearchStopped(_) => {}
				}
			}
			Err(_) => {
				println!("Error receiving MDNS event as the ServiceDaemon has been shut down. Local discovery has been disabled, please restart your app to re-enable local discovery!");
			}
		}
	}

	pub async fn register(&self) {
		let peer_id_str = &self.nm.peer_id.to_string();
		let service_info = ServiceInfo::new(
			&self.service_type,
			&peer_id_str,
			&format!("{}.", peer_id_str),
			self.nm
				.lan_addrs
				.iter()
				.map(|v| v.to_string())
				.collect::<Vec<_>>()
				.join(","),
			self.nm.listen_addr.port(),
			Some(self.nm.manager.get_metadata().to_hashmap()),
		);

		match service_info {
			Ok(service_info) => match self.mdns.register(service_info) {
				Ok(_) => {}
				Err(err) => {
					// println!("sd-p2p warning: failed to register service: {}", err);
					todo!(); // TODO
				}
			},
			Err(err) => {
				// println!("sd-p2p warning: failed to register service: {}", err);
				todo!(); // TODO
			}
		}
	}

	/// shutdown shuts down the MDNS service. This will advertise the current peer as unavailable to the rest of the network.
	pub(crate) fn shutdown(self: Arc<Self>) {
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
