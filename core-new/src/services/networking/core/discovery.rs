//! Peer discovery utilities

use crate::services::networking::{NetworkingError, Result};
use libp2p::{Multiaddr, PeerId};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Discovery mechanism types
#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
	Mdns,
	Dht,
	Manual,
}

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
	pub peer_id: PeerId,
	pub addresses: Vec<Multiaddr>,
	pub discovery_method: DiscoveryMethod,
	pub discovered_at: Instant,
	pub last_seen: Instant,
}

/// Peer discovery manager
pub struct DiscoveryManager {
	/// Map of peer ID to discovery info
	discovered_peers: HashMap<PeerId, DiscoveredPeer>,

	/// Maximum age for discovered peers
	max_peer_age: Duration,
}

impl DiscoveryManager {
	/// Create a new discovery manager
	pub fn new() -> Self {
		Self {
			discovered_peers: HashMap::new(),
			max_peer_age: Duration::from_secs(300), // 5 minutes
		}
	}

	/// Add a discovered peer
	pub fn add_discovered_peer(
		&mut self,
		peer_id: PeerId,
		addresses: Vec<Multiaddr>,
		method: DiscoveryMethod,
	) {
		let now = Instant::now();

		if let Some(existing) = self.discovered_peers.get_mut(&peer_id) {
			// Update existing peer
			existing.addresses = addresses;
			existing.last_seen = now;
		} else {
			// Add new peer
			let peer = DiscoveredPeer {
				peer_id,
				addresses,
				discovery_method: method,
				discovered_at: now,
				last_seen: now,
			};

			self.discovered_peers.insert(peer_id, peer);
		}
	}

	/// Remove a peer
	pub fn remove_peer(&mut self, peer_id: &PeerId) {
		self.discovered_peers.remove(peer_id);
	}

	/// Get all discovered peers
	pub fn get_discovered_peers(&self) -> Vec<&DiscoveredPeer> {
		self.discovered_peers.values().collect()
	}

	/// Get a specific peer
	pub fn get_peer(&self, peer_id: &PeerId) -> Option<&DiscoveredPeer> {
		self.discovered_peers.get(peer_id)
	}

	/// Clean up expired peers
	pub fn cleanup_expired(&mut self) {
		let now = Instant::now();

		self.discovered_peers
			.retain(|_, peer| now.duration_since(peer.last_seen) < self.max_peer_age);
	}

	/// Get peers discovered by a specific method
	pub fn get_peers_by_method(&self, method: DiscoveryMethod) -> Vec<&DiscoveredPeer> {
		self.discovered_peers
			.values()
			.filter(|peer| matches!(peer.discovery_method.clone(), method))
			.collect()
	}

	/// Get peer count
	pub fn peer_count(&self) -> usize {
		self.discovered_peers.len()
	}
}

impl Default for DiscoveryManager {
	fn default() -> Self {
		Self::new()
	}
}
