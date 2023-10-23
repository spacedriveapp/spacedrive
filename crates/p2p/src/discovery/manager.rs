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
	pub(crate) state: Arc<RwLock<DiscoveryManagerState>>,
	pub(crate) listen_addrs: HashSet<SocketAddr>,

	// TODO: Make this owned by the manager by splitting state of this
	pub(crate) mdns: Option<Mdns>,
}

impl DiscoveryManager {
	pub(crate) fn new(state: Arc<RwLock<DiscoveryManagerState>>) -> Self {
		// TODO
		// let (mdns, mdns_state) = Mdns::new(application_name, peer_id, metadata_manager)
		// 	.await
		// 	.unwrap();

		Self {
			state,
			listen_addrs: Default::default(),
			mdns: Some(Mdns),
		}
	}

	/// rebroadcast is called on changes to `self.services` to make sure all providers update their records
	pub(crate) fn rebroadcast(&self) {
		// todo!();
	}

	pub(crate) async fn register_addr(&mut self, addr: SocketAddr) {
		self.listen_addrs.insert(addr);
		self.rebroadcast();
	}

	pub(crate) async fn unregister_addr(&mut self, addr: &SocketAddr) {
		self.listen_addrs.remove(addr);
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
