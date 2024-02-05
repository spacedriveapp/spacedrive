use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::{HookId, ListenerId, RemoteIdentity, UnicastStream, P2P};

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
	// TODO: `pub removed: AtomicBool,` // TODO: This should disable methods on this `Peer` instance cause it can be cloned outta the system.
}

#[derive(Debug, Default)]
pub(crate) struct State {
	/// Active connections with the remote
	pub(crate) active_connections: HashMap<ListenerId, oneshot::Sender<()>>,
	/// Methods for establishing an active connections with the remote
	/// These should be inject by `Listener::acceptor` which is called when a new peer is discovered.
	pub(crate) connection_methods: HashMap<ListenerId, mpsc::Sender<RemoteIdentity>>,
	/// Methods that have discovered this peer.
	pub(crate) discovered: HashSet<HookId>,
}

impl State {
	pub(crate) fn needs_removal(&self) -> bool {
		self.discovered.is_empty()
			&& self.connection_methods.is_empty()
			&& self.active_connections.is_empty()
	}
}

impl Eq for Peer {}
impl PartialEq for Peer {
	fn eq(&self, other: &Self) -> bool {
		self.identity == other.identity
	}
}

// Internal methods
impl Peer {
	pub(crate) fn new(identity: RemoteIdentity, p2p: Arc<P2P>) -> Arc<Self> {
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

	pub fn discovered_by(&self, hook: HookId) {
		self.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.discovered
			.insert(hook);
	}

	pub fn can_connect(&self) -> bool {
		!self
			.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.connection_methods
			.is_empty()
	}

	pub fn is_connected(&self) -> bool {
		!self
			.state
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.active_connections
			.is_empty()
	}

	/// Construct a new Quic stream to the peer.
	pub async fn new_stream(&self) -> Result<UnicastStream, ()> {
		// self.state
		// 	.read()
		// 	.unwrap_or_else(PoisonError::into_inner)
		// 	.connection_methods
		// 	.iter()
		// 	.next()
		// 	.map(|(id, tx)| {
		// 		let (tx, rx) = oneshot::channel();
		// 		self.state
		// 			.write()
		// 			.unwrap_or_else(PoisonError::into_inner)
		// 			.active_connections
		// 			.insert(*id, tx);
		// 		Ok(UnicastStream::new(rx))
		// 	})
		todo!();
	}
}

// Hook-facing methods
impl Peer {
	pub fn connected_to(&self, listener: ListenerId, shutdown_tx: oneshot::Sender<()>) {
		let Some(p2p) = self.p2p.upgrade() else {
			warn!(
				"P2P System holding peer '{:?}' despite system being dropped",
				self.identity
			);
			return;
		};
	}
}
