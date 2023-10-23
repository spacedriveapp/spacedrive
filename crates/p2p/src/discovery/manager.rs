use std::{
	collections::{HashMap, HashSet},
	net::SocketAddr,
	sync::{Arc, PoisonError, RwLock},
};

use crate::{spacetunnel::RemoteIdentity, PeerId};

type ServiceName = String;

// TODO: Should this be public or hidden behind `Manager`?

pub struct Mdns; // TODO: This is a placeholder

/// DiscoveryManager controls all user-defined [Service]'s and connects them with the network through mDNS and other discovery protocols
pub struct DiscoveryManager {
	pub(crate) state: RwLock<DiscoveryManagerState>,
	pub(crate) listen_addrs: RwLock<HashSet<SocketAddr>>,

	// TODO: Make this owned by the manager by splitting state of this
	pub(crate) mdns: Option<Mdns>,
}

impl DiscoveryManager {
	pub(crate) fn new() -> Arc<Self> {
		// TODO: listen_addrs

		Arc::new(Self {
			state: Default::default(),
			listen_addrs: Default::default(),
			mdns: Some(Mdns),
		})
	}

	/// rebroadcast is called on changes to `self.services` to make sure all providers update their records
	pub(crate) fn rebroadcast(&self) {
		// todo!();
	}

	pub(crate) async fn register_addr(&self, addr: SocketAddr) {
		self.listen_addrs
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(addr);
		self.rebroadcast();
	}

	pub(crate) async fn unregister_addr(&self, addr: &SocketAddr) {
		self.listen_addrs
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(addr);
		self.rebroadcast();
	}

	pub(crate) fn shutdown(&self) {
		// todo!();
	}
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DiscoveryManagerState {
	/// A list of services the current node is advertising
	pub(crate) services: HashMap<ServiceName, HashMap<String, String>>,
	/// A map of organically discovered peers
	pub(crate) discovered: HashMap<ServiceName, HashMap<RemoteIdentity, RemotePeer>>,
	/// A map of peers we know about. These may be connected or not avaiable.
	/// This is designed around the Relay/NAT hole punching service where we need to emit who we wanna discover
	pub(crate) known: HashMap<ServiceName, HashMap<RemoteIdentity, RemotePeer>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RemotePeerCandidate {
	pub(crate) peer_id: PeerId,
	pub(crate) meta: HashMap<String, String>,
	pub(crate) addresses: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
pub(crate) enum RemotePeer {
	Unavailable,
	Discovered(RemotePeerCandidate),
	Connected(RemotePeerCandidate),
}
