use std::{
	collections::{HashMap, HashSet},
	future::poll_fn,
	net::SocketAddr,
	sync::{Arc, PoisonError, RwLock},
	task::Poll,
};

use libp2p::PeerId;
use tokio::sync::{broadcast, mpsc};
use tracing::trace;

use crate::{spacetunnel::RemoteIdentity, ManagerConfig, ServiceEventInternal};

type ServiceName = String;

pub type ListenAddrs = HashSet<SocketAddr>;
pub type State = Arc<RwLock<DiscoveryManagerState>>;

// /// `DiscoveryManager` controls all user-defined [Service]'s and connects them with the network through mDNS and other discovery protocols
// pub struct DiscoveryManager {
// 	pub(crate) state: State,
// 	pub(crate) listen_addrs: ListenAddrs,
// 	pub(crate) application_name: &'static str,
// 	pub(crate) identity: RemoteIdentity,
// 	pub(crate) peer_id: PeerId,
// 	pub(crate) mdns: Option<Mdns>,
// 	// TODO: Split these off `DiscoveryManagerState` and parse around on their own struct???
// 	pub(crate) do_broadcast_rx: broadcast::Receiver<()>,
// 	pub(crate) service_shutdown_rx: mpsc::Receiver<String>,
// }

#[derive(Debug, Clone)]
#[allow(clippy::type_complexity)]
pub struct DiscoveryManagerState {
	/// A list of services the current node is advertising w/ their metadata
	pub(crate) services: HashMap<
		ServiceName,
		(
			broadcast::Sender<(String, ServiceEventInternal)>,
			// Will be `None` prior to the first `.set` call
			Option<HashMap<String, String>>,
		),
	>,
	/// A map of organically discovered peers
	pub(crate) discovered: HashMap<ServiceName, HashMap<RemoteIdentity, DiscoveredPeerCandidate>>,
	/// A map of peers we know about. These may be connected or not avaiable.
	/// This is designed around the Relay/NAT hole punching service where we need to emit who we wanna discover
	/// Note: this may contain duplicates with `discovered` as they will *not* be removed from here when found
	pub(crate) known: HashMap<ServiceName, HashSet<RemoteIdentity>>,
	/// Used to trigger an rebroadcast. This should be called when mutating this struct.
	/// You are intended to clone out of this instead of locking the whole struct's `RwLock` each time you wanna use it.
	/// This is a channel with a capacity of 1. If sending fails we know someone else has already requested broadcast and we can ignore the error.
	pub(crate) do_broadcast: broadcast::Sender<()>,
	/// Used to trigger the removal of a `Service`. This is used in the `impl Drop for Service`
	/// You are intended to clone out of this instead of locking the whole struct's `RwLock` each time you wanna use it.
	pub(crate) service_shutdown_tx: mpsc::Sender<String>,
}

impl DiscoveryManagerState {
	#[must_use]
	pub fn new() -> (Arc<RwLock<Self>>, mpsc::Receiver<String>) {
		let (service_shutdown_tx, service_shutdown_rx) = mpsc::channel(10);

		(
			Arc::new(RwLock::new(Self {
				services: Default::default(),
				discovered: Default::default(),
				known: Default::default(),
				do_broadcast: broadcast::channel(1).0,
				service_shutdown_tx,
			})),
			service_shutdown_rx,
		)
	}
}

#[derive(Debug, Clone)]
pub struct DiscoveredPeerCandidate {
	pub(crate) peer_id: PeerId,
	pub(crate) meta: HashMap<String, String>,
	pub(crate) addresses: Vec<SocketAddr>,
}
