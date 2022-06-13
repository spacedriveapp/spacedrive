use std::{process, sync::Arc};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::broadcast;

use crate::{
	DiscoveryManager, NetworkManagerError, NetworkManagerEvent, PeerCandidate, PeerId, PeerMetadata,
};

/// MDNS is used to discover other peers on the local network using the mDNS protocol.
/// Refer to [RFC 6762](https://datatracker.ietf.org/doc/html/rfc6762) and [RFC 6763](https://datatracker.ietf.org/doc/html/rfc6763) for more information about the protocol behind this system.
pub(crate) struct MDNS {
	/// discovery is the Discovery manager. mDNS is a provider for mDNS events which are stored in the discovery manager.
	discovery: Arc<DiscoveryManager>,
	/// service_type is the type of service we want to advertise. This should be in the form '_appname._udp.local.'. This must follow RFC 6763 section 7 "Service Names".
	service_type: String,
	/// mdns is the mDNS service daemon that is used to discover other nodes on the local network.
	mdns: ServiceDaemon,
}

impl MDNS {
	pub async fn init(
		app_name: &'static str,
		discovery: Arc<DiscoveryManager>,
		mut discovery_channel: broadcast::Receiver<()>,
	) -> Result<(), NetworkManagerError> {
		let this = Arc::new(Self {
			discovery,
			service_type: format!("_{}._udp.local.", app_name),
			mdns: ServiceDaemon::new().map_err(|err| NetworkManagerError::MDNSDaemon(err))?,
		});

		let this2 = this.clone();
		ctrlc::set_handler(move || {
			this2.clone().shutdown();
			process::exit(0);
		})
		.map_err(|err| NetworkManagerError::ShutdownHandler(err))?;

		let browser = this
			.mdns
			.browse(&this.service_type)
			.map_err(|err| NetworkManagerError::MDNSDaemon(err))?;

		tokio::spawn(async move {
			loop {
				tokio::select! {
					event = browser.recv_async() => {
						match event {
							Ok(event) => match this.clone().handle_mdns_event(event).await {
								Ok(_) => {},
								Err(_) => break,
							},
							Err(_) => {
								println!("sd-p2p warning: 'mdns' channel has been shut down! You will not receive any more events!");
								break;
							}
						}
					}
					event = discovery_channel.recv() => {
						match event {
							Ok(_) => this.clone().register_service(),
							Err(_) => {
								println!("sd-p2p warning: 'discovery_channel' channel has been shut down! You will not receive any more events!");
								break;
							}
						}
					}
				}
			}
		});

		Ok(())
	}

	/// handle_mdns_event is called when a new event is received from the 'mdns' listener.
	pub(crate) async fn handle_mdns_event(self: Arc<Self>, event: ServiceEvent) -> Result<(), ()> {
		match event {
			ServiceEvent::SearchStarted(_) => {}
			ServiceEvent::ServiceFound(_, _) => {}
			ServiceEvent::ServiceResolved(info) => {
				let raw_peer_id = info
					.get_fullname()
					.replace(&format!(".{}", self.service_type), "");

				// Prevent discovery of the current node.
				if raw_peer_id == self.discovery.server.peer_id.to_string() {
					return Ok(());
				}

				match PeerId::from_str(raw_peer_id.clone()) {
					Ok(peer_id) => {
						let peer = PeerCandidate {
							id: peer_id.clone(),
							metadata: PeerMetadata::from_hashmap(&peer_id, info.get_properties()),
							addresses: info
								.get_addresses()
								.iter()
								.map(|addr| addr.clone())
								.collect(),
							port: info.get_port(),
						};

						let is_peer_connected = self
							.discovery
							.server
							.connected_peers
							.read()
							.await
							.contains_key(&peer_id);
						self.discovery
							.discovered_peers
							.insert(peer_id, peer.clone());

						if !is_peer_connected {
							self.discovery
                                .server
                                .application_channel
                                .send(NetworkManagerEvent::PeerDiscovered { peer })
                                .await
                                .map_err(|_| {
                                    println!("sd-p2p warning: 'application_channel' channel has been shut down! You will not receive any more events!");
                                    ()
                                })?;
						}
					}
					Err(_) => {
						println!("sd-p2p warning: resolved node advertising itself with an invalid peer_id '{}'", raw_peer_id);
					}
				}
			}
			ServiceEvent::ServiceRemoved(_, fullname) => {
				let raw_peer_id = fullname.replace(&format!(".{}", self.service_type), "");

				// Prevent discovery of the current node.
				if raw_peer_id == self.discovery.server.peer_id.to_string() {
					return Ok(());
				}

				match PeerId::from_str(raw_peer_id.clone()) {
					Ok(peer_id) => {
						self.discovery.discovered_peers.remove(&peer_id);
					}
					Err(_) => {
						println!("sd-p2p warning: removing node advertising itself with an invalid peer_id '{}'", raw_peer_id);
					}
				}
			}
			ServiceEvent::SearchStopped(_) => {}
		}

		Ok(())
	}

	/// register_service will register the current node with the mDNS service daemon. This is run every time a network interface is updated on the peer so that the remote nodes can determine all possible routes to the current node.
	pub fn register_service(self: Arc<Self>) {
		let service_info = ServiceInfo::new(
			&self.service_type,
			&self.discovery.server.peer_id.to_string(),
			&format!("{}.", self.discovery.server.peer_id.to_string()),
			self.discovery
				.local_addrs
				.iter()
				.map(|v| v.to_string())
				.collect::<Vec<_>>()
				.join(","),
			self.discovery.server.listen_addr.port(),
			Some(self.discovery.p2p_application.get_metadata().to_hashmap()),
		);

		match service_info {
			Ok(service_info) => match self.mdns.register(service_info) {
				Ok(_) => {}
				Err(err) => {
					println!("sd-p2p warning: failed to register service: {}", err);
				}
			},
			Err(err) => {
				println!("sd-p2p warning: failed to register service: {}", err);
			}
		}
	}

	/// shutdown shuts down the MDNS service. This will advertise the current peer as unavailable to the rest of the network.
	pub(crate) fn shutdown(self: Arc<Self>) {
		// The panics caused by `.expect` are acceptable here because they are run during shutdown where nothing can be done if they were to fail.

		self.mdns
			.unregister(&format!(
				"{}.{}",
				self.discovery.server.peer_id, self.service_type
			))
			.expect("Error unregistering the mDNS service")
			.recv()
			.expect("Error unregistering the mDNS service");

		self.mdns
			.shutdown()
			.expect("Error shutting down mDNS service");
	}
}
