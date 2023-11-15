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

use crate::{spacetunnel::RemoteIdentity, ManagerConfig, Mdns, ServiceEventInternal};

type ServiceName = String;

pub type ListenAddrs = HashSet<SocketAddr>;
pub type State = Arc<RwLock<DiscoveryManagerState>>;

/// `DiscoveryManager` controls all user-defined [Service]'s and connects them with the network through mDNS and other discovery protocols
pub struct DiscoveryManager {
	pub(crate) state: State,
	pub(crate) listen_addrs: ListenAddrs,
	pub(crate) application_name: &'static str,
	pub(crate) identity: RemoteIdentity,
	pub(crate) peer_id: PeerId,
	pub(crate) mdns: Option<Mdns>,
	// TODO: Split these off `DiscoveryManagerState` and parse around on their own struct???
	pub(crate) do_broadcast_rx: broadcast::Receiver<()>,
	pub(crate) service_shutdown_rx: mpsc::Receiver<String>,
}

impl DiscoveryManager {
	pub(crate) fn new(
		application_name: &'static str,
		identity: RemoteIdentity,
		peer_id: PeerId,
		config: &ManagerConfig,
		state: State,
		service_shutdown_rx: mpsc::Receiver<String>,
	) -> Result<Self, mdns_sd::Error> {
		let mut mdns = None;
		if config.enabled {
			mdns = Some(Mdns::new(application_name, identity, peer_id)?);
		}

		let do_broadcast_rx = state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.do_broadcast
			.subscribe();

		Ok(Self {
			state,
			listen_addrs: Default::default(),
			application_name,
			identity,
			peer_id,
			mdns,
			do_broadcast_rx,
			service_shutdown_rx,
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
			 _ = self.do_broadcast_rx.recv() => self.do_advertisement(),
			service_name = self.service_shutdown_rx.recv() => {
				if let Some(service_name) = service_name {
					let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
					state.services.remove(&service_name);
					state.discovered.remove(&service_name);
					state.known.remove(&service_name);
				}

				// TODO

				self.do_advertisement();
			}
			() = poll_fn(|cx| {
				if let Some(mdns) = &mut self.mdns {
					return mdns.poll(cx, &self.listen_addrs, &self.state);
				}

				Poll::Pending
			}) => {},
		}
	}

	pub(crate) fn shutdown(&self) {
		if let Some(mdns) = &self.mdns {
			mdns.shutdown();
		}
	}
}

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
	#[must_use] pub fn new() -> (Arc<RwLock<Self>>, mpsc::Receiver<String>) {
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
