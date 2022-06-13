use std::{
	net::{IpAddr, Ipv4Addr},
	pin::Pin,
	sync::Arc,
};

mod mdns;

use dashmap::{DashMap, DashSet};
use if_watch::{IfEvent, IfWatcher};
use tokio::sync::broadcast;

use crate::{
	server::Server, NetworkManagerError, P2PApplication, PeerCandidate, PeerId, PeerMetadata,
};

use self::mdns::MDNS;

/// DiscoveryManager is responsible for discovering other peers on the network.
pub(crate) struct DiscoveryManager {
	/// TODO
	server: Arc<Server>,
	/// local_addrs is a list of all the IP addresses that are associated with the current node.
	local_addrs: DashSet<Ipv4Addr>,
	/// discovery_channel is called whenever a change is made to the [local_addrs] map. This will trigger all discovery systems (mDNS or Global) to publish this change to other peers in the network.
	discovery_channel: broadcast::Sender<()>,
	/// discovered_peers is a map of all peers that has been discovered on your local network. Be aware a peer could be offline and remain in this map for many minutes but it will eventually be removed once the peer is detected to be offline.
	pub(crate) discovered_peers: DashMap<PeerId, PeerCandidate>,
	/// p2p_application is a trait implemented by the application embedded the network manager. This allows the application to take control of the actions of the network manager.
	pub(crate) p2p_application: Arc<dyn P2PApplication + Send + Sync>,
}

impl DiscoveryManager {
	pub async fn new(
		app_name: &'static str,
		server: Arc<Server>,
		p2p_application: Arc<dyn P2PApplication + Send + Sync>,
	) -> Result<Arc<Self>, NetworkManagerError> {
		let mut if_watcher = IfWatcher::new()
			.await
			.map_err(|err| NetworkManagerError::IfWatch(err))?;

		let (tx, rx) = broadcast::channel(25);
		let this = Arc::new(Self {
			server,
			local_addrs: if_watcher
				.iter()
				.filter_map(|iface| match iface.addr() {
					IpAddr::V4(ip) => {
						if ip == Ipv4Addr::LOCALHOST {
							None
						} else {
							Some(ip)
						}
					}
					IpAddr::V6(_) => None,
				})
				.collect::<DashSet<_>>(),
			discovery_channel: tx,
			discovered_peers: DashMap::new(),
			p2p_application,
		});

		// Mount providers
		MDNS::init(app_name, this.clone(), rx).await?;

		// Run discovery thread
		let this2 = this.clone();
		tokio::spawn(async move {
			loop {
				match Pin::new(&mut if_watcher).await {
					Ok(event) => match this2.clone().handle_ifwatch_event(event).await {
						Ok(_) => {}
						Err(_) => break, // Shutdown thread when the `discovery_channel` is closed.
					},
					Err(_) => {
						println!("sd-p2p warning: 'if_watcher' channel returned an error!");
						break; // Shutdown thread when the `if_watcher` channel is closed.
					}
				}
			}
		});

		Ok(this)
	}

	async fn handle_ifwatch_event(self: Arc<Self>, event: IfEvent) -> Result<(), ()> {
		match event {
			IfEvent::Up(iface) => match iface.addr() {
				IpAddr::V4(ip) => {
					if ip != Ipv4Addr::LOCALHOST {
						self.local_addrs.insert(ip);
						self.discovery_channel.send(()).map_err(|_| {
                            println!("sd-p2p warning: 'discovery_channel' channel has been shut down! You will not receive any more events!");
                            ()
                        })?;
					}
				}
				IpAddr::V6(_) => {}
			},
			IfEvent::Down(iface) => match iface.addr() {
				IpAddr::V4(ip) => {
					if ip != Ipv4Addr::LOCALHOST {
						self.local_addrs.remove(&ip);
						self.discovery_channel.send(()).map_err(|_| {
                            println!("sd-p2p warning: 'discovery_channel' channel has been shut down! You will not receive any more events!");
                            ()
                        })?;
					}
				}
				IpAddr::V6(_) => {}
			},
		}

		Ok(())
	}
}
