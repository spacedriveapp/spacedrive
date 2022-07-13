use std::sync::Arc;

use sd_tunnel_utils::{Client, Message};

use crate::{NetworkManager, NetworkManagerError, P2PManager};

/// GlobalDiscovery is the discovery system for discovering devices which are not on the same local network as you.
/// This is done through the Spacetunnel server hosted by Spacedrive Inc. it could however be hosted by anyone and documentation for doing so will be released in the future once we are confident in the current design.
pub(crate) struct GlobalDiscovery<TP2PManager: P2PManager> {
	nm: Arc<NetworkManager<TP2PManager>>,
	client: Client,
}

impl<TP2PManager: P2PManager> GlobalDiscovery<TP2PManager> {
	pub fn init(nm: &Arc<NetworkManager<TP2PManager>>) -> Result<Self, NetworkManagerError> {
		if let Some(url) = &nm.spacetunnel_url {
			Ok(Self {
				nm: nm.clone(),
				client: Client::new(url.clone(), nm.endpoint.clone(), nm.identity.clone()),
			})
		} else {
			panic!("Why no Spacetunnel? (~_^)");
			// TODO: Refactor to allow the system to work without Spacetunnel enabled.
		}
	}

	pub async fn poll(&self) {
		// TODO: Allow the tunnel server to accept a list of PeerId's instead of doing heaps of requests
		let peers = self.nm.known_peers.iter().map(|v| v.clone()).collect();
		let msg = self
			.client
			.send_message(Message::QueryClientAnnouncement(peers))
			.await
			.map_err(|err| {
				println!(
					"[TODO: WIP FEATURE REPORTED ERROR] Spacetunnel failed lookup peers with error: {:?}",
					err
				)
			});

		// TODO: Handle error from discovery service
		// println!("{:?}", msg);
		// self.nm.discovered_peers.insert(key, value); // TODO: make this work
		// TODO: Open connection to peers if they are not already connected
	}

	pub async fn register(&self) {
		// TODO: Send the metadata along with the discovery payload
		// TODO: Only do announcement if data has changed or it's been over 10 minutes since last packet

		let msg = self
			.client
			.send_message(Message::ClientAnnouncement {
				peer_id: self.nm.peer_id.clone(),
				addresses: self.nm.lan_addrs.iter().map(|v| v.to_string()).collect(), // TODO: Include STUN address in this list
			})
			.await
			.map_err(|err| {
				println!("[TODO: WIP FEATURE REPORTED ERROR] Spacetunnel failed announcement with error: {:?}", err)
			});

		// // TODO: Handle error from discovery service
		// println!("{:?}", msg);
	}

	pub(crate) fn shutdown(&self) {
		// TODO: Remove the announcement from the tunnel
	}
}
