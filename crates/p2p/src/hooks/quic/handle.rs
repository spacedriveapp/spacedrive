use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::Notify;

use crate::{HookId, PeerConnectionCandidate, RemoteIdentity, P2P};

/// A handle to the QUIC hook.
///
/// This allows for manually registering peers, which is required so that we can ask the relay to connect to them.
#[derive(Debug)]
pub struct QuicHandle {
	pub(super) shutdown: Notify,
	pub(super) p2p: Arc<P2P>,
	pub(super) hook_id: HookId,
	pub(super) nodes: Mutex<HashSet<RemoteIdentity>>,
}

impl QuicHandle {
	/// A future that resolves when the QUIC hook is shut down.
	pub async fn shutdown(&self) {
		self.shutdown.notified().await
	}

	/// add a new peer to be tracked.
	///
	/// This will allow the relay to connect to it.
	pub fn track_peer(&self, identity: RemoteIdentity, metadata: HashMap<String, String>) {
		self.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(identity.clone());

		self.p2p.clone().discover_peer(
			self.hook_id,
			identity,
			metadata,
			[PeerConnectionCandidate::Relay].into_iter().collect(),
		);
	}

	/// remove a peer from being tracked.
	///
	/// This will stop the relay from trying to connect to it.
	pub fn untrack_peer(&self, identity: RemoteIdentity) {
		self.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&identity);

		self.p2p
			.peers()
			.get(&identity)
			.map(|peer| peer.undiscover_peer(self.hook_id));
	}

	/// check if a peer is being relayed.
	pub fn is_relayed(&self, identity: RemoteIdentity) -> bool {
		self.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.get(&identity)
			.is_some()
	}
}
