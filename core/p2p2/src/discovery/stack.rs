use std::sync::Arc;

use crate::{GlobalDiscovery, NetworkManager, NetworkManagerError, P2PManager, MDNS};

/// Represents a stack of all of the different discovery mechanisms that are used by the P2P library.
/// Traits are not used due to Rust's current lack of proper support for async traits.
pub(crate) struct DiscoveryStack<TP2PManager: P2PManager> {
	pub mdns: Arc<MDNS<TP2PManager>>,
	pub global: Arc<GlobalDiscovery<TP2PManager>>,
}

impl<TP2PManager: P2PManager> DiscoveryStack<TP2PManager> {
	pub async fn new(nm: &Arc<NetworkManager<TP2PManager>>) -> Result<Self, NetworkManagerError> {
		let global = Arc::new(GlobalDiscovery::init(nm)?);
		global.poll().await;

		Ok(Self {
			mdns: Arc::new(MDNS::init(nm)?),
			global,
		})
	}

	pub async fn register(&self) {
		self.mdns.register().await;
		self.global.register().await;
	}

	pub fn shutdown(&self) {
		self.mdns.shutdown();
		self.global.shutdown();
	}
}
