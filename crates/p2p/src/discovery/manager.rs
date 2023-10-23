use std::{
	collections::HashMap,
	net::SocketAddr,
	sync::{Arc, RwLock},
};

use crate::{spacetunnel::RemoteIdentity, PeerId};

type ServiceName = String;

// TODO: Should this be public or hidden behind `Manager`?

/// DiscoveryManager controls all user-defined [Service]'s and connects them with the network through mDNS and other discovery protocols
pub struct DiscoveryManager {
	pub(crate) state: RwLock<DiscoveryManagerState>,
}

impl DiscoveryManager {
	pub(crate) fn new() -> Arc<Self> {
		Arc::new(Self {
			state: Default::default(),
		})
	}

	/// rebroadcast is called on changes to `self.services` to make sure all providers update their records
	pub(crate) fn rebroadcast(&self) {
		todo!();
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
