use std::{
	collections::{HashMap, HashSet},
	future::poll_fn,
	net::SocketAddr,
	sync::{Arc, PoisonError, RwLock},
	task::Poll,
};

use tokio::sync::{broadcast, Notify};
use tracing::trace;

use crate::{spacetunnel::RemoteIdentity, ManagerConfig, Mdns, PeerId};

type ServiceName = String;

pub(crate) type ListenAddrs = HashSet<SocketAddr>;
pub(crate) type State = Arc<RwLock<DiscoveryManagerState>>;

/// DiscoveryManager controls all user-defined [Service]'s and connects them with the network through mDNS and other discovery protocols
pub(crate) struct DiscoveryManager {
	pub(crate) state: State,
	pub(crate) listen_addrs: ListenAddrs,
	pub(crate) application_name: &'static str,
	pub(crate) peer_id: PeerId,
	pub(crate) mdns: Option<Mdns>,
	pub(crate) do_broadcast: Arc<Notify>,
}

impl DiscoveryManager {
	pub(crate) fn new(
		application_name: &'static str,
		peer_id: PeerId,
		config: &ManagerConfig,
		state: State,
	) -> Result<Self, mdns_sd::Error> {
		let mut mdns = None;
		if config.enabled {
			mdns = Some(Mdns::new(&application_name, peer_id)?);
		}

		let do_broadcast = state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.do_broadcast
			.clone();

		Ok(Self {
			state,
			listen_addrs: Default::default(),
			application_name,
			peer_id,
			mdns,
			do_broadcast,
		})
	}

	/// is called on changes to `self.services` to make sure all providers update their records
	pub(crate) fn do_advertisement(&mut self) {
		trace!("Broadcasting new service records");

		if let Some(mdns) = &mut self.mdns {
			mdns.do_advertisement(&self.listen_addrs, &self.state);
		}
	}

	pub(crate) async fn poll(&mut self) {
		tokio::select! {
			 _ = self.do_broadcast.notified() => self.do_advertisement(),
			_ = poll_fn(|cx| {
				if let Some(mdns) = &mut self.mdns {
					return mdns.poll(cx, &self.listen_addrs, &self.state);
				}

				Poll::Pending
			}) => self.do_advertisement(),
		}
	}

	pub(crate) fn shutdown(&self) {
		if let Some(mdns) = &self.mdns {
			mdns.shutdown();
		}
	}
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoveryManagerState {
	/// A list of services the current node is advertising w/ their metadata
	pub(crate) services: HashMap<ServiceName, (broadcast::Sender<()>, HashMap<String, String>)>,
	/// A map of organically discovered peers
	pub(crate) discovered: HashMap<ServiceName, HashMap<RemoteIdentity, DiscoveredPeerCandidate>>,
	/// A map of peers we know about. These may be connected or not avaiable.
	/// This is designed around the Relay/NAT hole punching service where we need to emit who we wanna discover
	/// Note: this may contain duplicates with `discovered` as they will *not* be removed from here when found
	pub(crate) known: HashMap<ServiceName, HashSet<RemoteIdentity>>,
	/// Used to trigger an rebroadcast. This should be called when mutating this struct.
	/// You are intended to clone out of this instead of locking the whole struct's `RwLock` each time you wanna use it.
	pub(crate) do_broadcast: Arc<Notify>,
}

impl Default for DiscoveryManagerState {
	fn default() -> Self {
		Self {
			services: Default::default(),
			discovered: Default::default(),
			known: Default::default(),
			do_broadcast: Default::default(),
		}
	}
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoveredPeerCandidate {
	pub(crate) peer_id: PeerId,
	pub(crate) meta: HashMap<String, String>,
	pub(crate) addresses: Vec<SocketAddr>,
}
