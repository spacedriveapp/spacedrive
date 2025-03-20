use std::{
	collections::{HashMap, HashSet},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Mutex, PoisonError,
	},
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
	pub(super) nodes: Mutex<HashMap<RemoteIdentity, HashMap<String, String>>>,
	pub(super) enabled: AtomicBool,
	pub(super) connected_via_relay: Mutex<HashSet<RemoteIdentity>>,
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
			.insert(identity, metadata.clone());

		if self.enabled.load(Ordering::Relaxed) {
			self.p2p.clone().discover_peer(
				self.hook_id,
				identity,
				metadata,
				[PeerConnectionCandidate::Relay].into_iter().collect(),
			);
		}
	}

	/// remove a peer from being tracked.
	///
	/// This will stop the relay from trying to connect to it.
	pub fn untrack_peer(&self, identity: RemoteIdentity) {
		self.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&identity);

		if self.enabled.load(Ordering::Relaxed) {
			if let Some(peer) = self.p2p.peers().get(&identity) {
				peer.undiscover_peer(self.hook_id)
			}
		}
	}

	/// remove all peers from being tracked.
	pub fn untrack_all(&self) {
		let mut nodes = self.nodes.lock().unwrap_or_else(PoisonError::into_inner);
		for (node, _) in nodes.drain() {
			if let Some(peer) = self.p2p.peers().get(&node) {
				peer.undiscover_peer(self.hook_id)
			}
		}
	}

	/// enabled the track peers from being registered to the P2P system.
	///
	/// This allows easily removing them when the relay is disabled.
	pub fn enable(&self) {
		self.enabled.store(true, Ordering::Relaxed);

		for (identity, metadata) in self
			.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
		{
			self.p2p.clone().discover_peer(
				self.hook_id,
				*identity,
				metadata.clone(),
				[PeerConnectionCandidate::Relay].into_iter().collect(),
			);
		}
	}

	/// disabled tracking the peers from being registered to the P2P system.
	pub fn disable(&self) {
		self.enabled.store(false, Ordering::Relaxed);

		for (identity, _) in self
			.nodes
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
		{
			if let Some(peer) = self.p2p.peers().get(identity) {
				peer.undiscover_peer(self.hook_id)
			}
		}
	}

	/// check if a peer is being relayed.
	pub fn is_relayed(&self, identity: RemoteIdentity) -> bool {
		self.connected_via_relay
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.get(&identity)
			.is_some()
	}
}
