use std::{
	collections::{BTreeSet, HashMap, HashSet},
	net::SocketAddr,
	sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

use crate::NewStreamError;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::{HookEvent, HookId, ListenerId, RemoteIdentity, UnicastStream, P2P};

#[derive(Debug)]
pub struct Peer {
	/// RemoteIdentity of the peer.
	pub(crate) identity: RemoteIdentity,
	/// Information from `P2P::service` on the remote node.
	pub(crate) metadata: RwLock<HashMap<String, String>>,
	/// We want these states to locked by the same lock so we can ensure they are consistent.
	pub(crate) state: RwLock<State>,
	/// A reference back to the P2P system.
	/// This is weak so we don't have recursive `Arc`'s that can never be dropped.
	pub(crate) p2p: Weak<P2P>,
}

// The order of this enum is the preference of the connection type.
#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum PeerConnectionCandidate {
	SocketAddr(SocketAddr),
	Relay,
	// Custom(String),
}

#[derive(Debug, Default)]
pub(crate) struct State {
	/// Active connections with the remote
	pub(crate) active_connections: HashMap<ListenerId, oneshot::Sender<()>>,
	/// Methods for establishing an active connections with the remote
	/// These should be inject by `Listener::acceptor` which is called when a new peer is discovered.
	pub(crate) connection_methods: HashMap<ListenerId, mpsc::Sender<ConnectionRequest>>,
	/// Methods that have discovered this peer.
	pub(crate) discovered: HashMap<HookId, BTreeSet<PeerConnectionCandidate>>,
}

/// A request to connect to a client.
/// This will be handled by a configured listener hook.
#[derive(Debug)]
#[non_exhaustive]
pub struct ConnectionRequest {
	pub to: RemoteIdentity,
	pub addrs: BTreeSet<PeerConnectionCandidate>,
	pub tx: oneshot::Sender<Result<UnicastStream, String>>,
}

// TODO: Maybe use this?
// impl State {
// 	pub(crate) fn needs_removal(&self) -> bool {
// 		self.discovered.is_empty()
// 			&& self.connection_methods.is_empty()
// 			&& self.active_connections.is_empty()
// 	}
// }

impl Eq for Peer {}
impl PartialEq for Peer {
	fn eq(&self, other: &Self) -> bool {
		self.identity == other.identity
	}
}

// Internal methods
impl Peer {
	// TODO: Make this private
	pub fn new(identity: RemoteIdentity, p2p: Arc<P2P>) -> Arc<Self> {
		Arc::new(Self {
			identity,
			metadata: Default::default(),
			state: Default::default(),
			p2p: Arc::downgrade(&p2p),
		})
	}
}

// User-facing methods
impl Peer {
	pub fn identity(&self) -> RemoteIdentity {
		self.identity
	}

	pub fn metadata(&self) -> RwLockReadGuard<HashMap<String, String>> {
		self.metadata.read().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn metadata_mut(&self) -> RwLockWriteGuard<HashMap<String, String>> {
		self.metadata
			.write()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn can_connect(&self) -> bool {
		!self
			.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.connection_methods
			.is_empty()
	}

	pub fn can_connect_with(&self, hook_id: HookId) -> bool {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.discovered
			.contains_key(&hook_id)
	}

	pub fn connection_candidates(&self) -> BTreeSet<PeerConnectionCandidate> {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.discovered
			.values()
			.flatten()
			.cloned()
			.collect()
	}

	pub fn is_connected(&self) -> bool {
		!self
			.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.active_connections
			.is_empty()
	}

	pub fn active_connections(&self) -> usize {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.active_connections
			.len()
	}

	// TODO: Possibly remove this, it's not great???
	pub fn is_connected_with_hook(&self, hook_id: HookId) -> bool {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.active_connections
			.contains_key(&ListenerId(hook_id.0))
	}

	pub fn is_connected_with(&self, listener_id: ListenerId) -> bool {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.active_connections
			.contains_key(&listener_id)
	}

	pub fn connection_methods(&self) -> HashSet<ListenerId> {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.connection_methods
			.keys()
			.copied()
			.collect()
	}

	pub fn discovered_by(&self) -> HashSet<HookId> {
		self.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.discovered
			.keys()
			.copied()
			.collect()
	}

	/// Construct a new Quic stream to the peer.
	pub async fn new_stream(&self) -> Result<UnicastStream, NewStreamError> {
		let (addrs, connect_tx) = {
			let state = self.state.read().unwrap_or_else(PoisonError::into_inner);

			let addrs = state
				.discovered
				.values()
				.flatten()
				.cloned()
				.collect::<BTreeSet<_>>();

			let Some((_id, connect_tx)) = state
				.connection_methods
				.iter()
				.map(|(id, tx)| (*id, tx.clone()))
				.next()
			else {
				return Err(NewStreamError::NoConnectionMethodsAvailable);
			};

			(addrs, connect_tx)
		};

		let (tx, rx) = oneshot::channel();
		connect_tx
			.send(ConnectionRequest {
				to: self.identity,
				addrs,
				tx,
			})
			.await
			.map_err(|err| {
				warn!("Failed to send connect request to peer: {}", err);
				NewStreamError::EventLoopOffline(err)
			})?;
		rx.await
			.map_err(|err| {
				warn!("Failed to receive connect response from peer: {err}");
				NewStreamError::ConnectionNeverEstablished(err)
			})?
			.map_err(|err| {
				warn!("Failed to do the thing: {err}");
				NewStreamError::Connecting(err)
			})
	}
}

// Hook-facing methods
impl Peer {
	pub fn hook_discovered(&self, hook: HookId, addrs: BTreeSet<PeerConnectionCandidate>) {
		// TODO: Emit event maybe???

		self.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.discovered
			.insert(hook, addrs);
	}

	pub fn listener_available(&self, listener: ListenerId, tx: mpsc::Sender<ConnectionRequest>) {
		self.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.connection_methods
			.insert(listener, tx);
	}

	pub fn undiscover_peer(&self, hook_id: HookId) {
		let Some(p2p) = self.p2p.upgrade() else {
			return;
		};

		let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
		state.discovered.remove(&hook_id);

		let hooks = p2p.hooks.read().unwrap_or_else(PoisonError::into_inner);
		hooks.iter().for_each(|(_, hook)| {
			hook.send(HookEvent::PeerExpiredBy(hook_id, self.identity));
		});

		if state.connection_methods.is_empty() && state.discovered.is_empty() {
			p2p.peers
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.remove(&self.identity);

			hooks.iter().for_each(|(_, hook)| {
				hook.send(HookEvent::PeerUnavailable(self.identity));
			});
		}
	}

	pub fn disconnected_from(&self, listener_id: ListenerId) {
		let Some(p2p) = self.p2p.upgrade() else {
			return;
		};

		let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
		state.connection_methods.remove(&listener_id);
		state.active_connections.remove(&listener_id);

		let hooks = p2p.hooks.read().unwrap_or_else(PoisonError::into_inner);
		hooks.iter().for_each(|(_, hook)| {
			hook.send(HookEvent::PeerDisconnectedWith(listener_id, self.identity));
		});

		if state.connection_methods.is_empty() && state.discovered.is_empty() {
			p2p.peers
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.remove(&self.identity);

			hooks.iter().for_each(|(_, hook)| {
				hook.send(HookEvent::PeerUnavailable(self.identity));
			});
		}
	}
}
