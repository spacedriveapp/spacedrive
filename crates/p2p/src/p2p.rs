use std::{
	collections::{hash_map::Entry, BTreeSet, HashMap, HashSet},
	net::SocketAddr,
	sync::{Arc, PoisonError, RwLock, RwLockReadGuard},
	time::Duration,
};

use flume::Sender;
use hash_map_diff::hash_map_diff;
use libp2p::futures::future::join_all;
use stable_vec::StableVec;
use tokio::time::timeout;
use tracing::info;

use crate::{
	hook::{HandlerFn, Hook, HookEvent, ListenerData, ListenerId, ShutdownGuard},
	smart_guards::SmartWriteGuard,
	HookId, Identity, Peer, PeerConnectionCandidate, RemoteIdentity, UnicastStream,
};

/// Manager for the entire P2P system.
#[derive(Debug)]
pub struct P2P {
	/// A unique identifier for the application.
	/// This will differentiate between different applications using this same P2P library.
	app_name: &'static str,
	/// The identity of the local node.
	/// This is the public/private keypair used to uniquely identify the node.
	identity: Identity,
	/// The channel is used by the application to handle incoming connections.
	/// Connection's are automatically closed when dropped so if user forgets to subscribe to this that will just happen as expected.
	handler_tx: Sender<UnicastStream>,
	/// Metadata is shared from the local node to the remote nodes.
	/// This will contain information such as the node's name, version, and services we provide.
	metadata: RwLock<HashMap<String, String>>,
	/// A list of all peers known to the P2P system. Be aware a peer could be connected and/or discovered at any time.
	pub(crate) peers: RwLock<HashMap<RemoteIdentity, Arc<Peer>>>,
	/// Hooks can be registered to react to state changes in the P2P system.
	pub(crate) hooks: RwLock<StableVec<Hook>>,
}

impl P2P {
	/// Construct a new P2P system.
	pub fn new(
		app_name: &'static str,
		identity: Identity,
		handler_tx: Sender<UnicastStream>,
	) -> Arc<Self> {
		app_name
			.chars()
			.all(|c| char::is_alphanumeric(c) || c == '-')
			.then_some(())
			.expect("'P2P::new': invalid app_name. Must be alphanumeric or '-' only.");
		#[allow(clippy::panic)]
		if app_name.len() > 12 {
			panic!("'P2P::new': app_name too long. Must be 12 characters or less.");
		}

		Arc::new(P2P {
			app_name,
			identity,
			metadata: Default::default(),
			peers: Default::default(),
			handler_tx,
			hooks: Default::default(),
		})
	}

	/// The unique identifier for this application.
	pub fn app_name(&self) -> &'static str {
		self.app_name
	}

	/// The identifier of this node that can *MUST* be kept secret.
	/// This is a private key in crypto terms.
	pub fn identity(&self) -> &Identity {
		&self.identity
	}

	/// The identifier of this node that can be shared.
	/// This is a public key in crypto terms.
	pub fn remote_identity(&self) -> RemoteIdentity {
		self.identity.to_remote_identity()
	}

	/// Metadata is shared from the local node to the remote nodes.
	/// This will contain information such as the node's name, version, and services we provide.
	pub fn metadata(&self) -> RwLockReadGuard<HashMap<String, String>> {
		self.metadata.read().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn metadata_mut(&self) -> SmartWriteGuard<HashMap<String, String>> {
		let lock = self
			.metadata
			.write()
			.unwrap_or_else(PoisonError::into_inner);

		SmartWriteGuard::new(self, lock, |p2p, before, after| {
			let diff = hash_map_diff(&before, after);
			if diff.updated.is_empty() && diff.removed.is_empty() {
				return;
			}

			p2p.hooks
				.read()
				.unwrap_or_else(PoisonError::into_inner)
				.iter()
				.for_each(|(_, hook)| {
					hook.send(HookEvent::MetadataModified);
				});
		})
	}

	/// A list of all peers known to the P2P system. Be aware a peer could be connected and/or discovered at any time.
	pub fn peers(&self) -> RwLockReadGuard<HashMap<RemoteIdentity, Arc<Peer>>> {
		self.peers.read().unwrap_or_else(PoisonError::into_inner)
	}

	// TODO: Should this take `addrs`???, A connection through the Relay probs doesn't have one in the same form.
	pub fn discover_peer(
		self: Arc<Self>,
		hook_id: HookId,
		identity: RemoteIdentity,
		metadata: HashMap<String, String>,
		addrs: BTreeSet<PeerConnectionCandidate>,
	) -> Arc<Peer> {
		let mut peers = self.peers.write().unwrap_or_else(PoisonError::into_inner);
		let peer = peers.entry(identity);
		let was_peer_inserted = matches!(peer, Entry::Vacant(_));
		let peer = peer
			.or_insert_with({
				let p2p = self.clone();
				|| Peer::new(identity, p2p)
			})
			.clone();

		let addrs = {
			let mut state = peer.state.write().unwrap_or_else(PoisonError::into_inner);
			let a = state.discovered.entry(hook_id).or_default();
			a.extend(addrs);
			a.clone()
		};

		peer.metadata_mut().extend(metadata);

		{
			let hooks = self.hooks.read().unwrap_or_else(PoisonError::into_inner);
			hooks
				.iter()
				.for_each(|(id, hook)| hook.acceptor(ListenerId(id), &peer, &addrs));

			if was_peer_inserted {
				hooks
					.iter()
					.for_each(|(_, hook)| hook.send(HookEvent::PeerAvailable(peer.clone())));
			}

			hooks.iter().for_each(|(_, hook)| {
				hook.send(HookEvent::PeerDiscoveredBy(hook_id, peer.clone()))
			});
		}

		peer
	}

	pub fn connected_to_incoming(
		self: Arc<Self>,
		listener: ListenerId,
		metadata: HashMap<String, String>,
		stream: UnicastStream,
	) -> Arc<Peer> {
		let peer = self
			.clone()
			.connected_to_outgoing(listener, metadata, stream.remote_identity());
		let _ = self.handler_tx.send(stream);
		peer
	}

	pub fn connected_to_outgoing(
		self: Arc<Self>,
		listener: ListenerId,
		metadata: HashMap<String, String>,
		identity: RemoteIdentity,
	) -> Arc<Peer> {
		let mut peers = self.peers.write().unwrap_or_else(PoisonError::into_inner);
		let peer = peers.entry(identity);
		let was_peer_inserted = matches!(peer, Entry::Vacant(_));
		let peer = peer
			.or_insert_with({
				let p2p = self.clone();
				move || Peer::new(identity, p2p)
			})
			.clone();

		{
			let mut state = peer.state.write().unwrap_or_else(PoisonError::into_inner);
			state.active_connections.insert(listener);
		}

		peer.metadata_mut().extend(metadata);

		{
			let hooks = self.hooks.read().unwrap_or_else(PoisonError::into_inner);

			if was_peer_inserted {
				hooks
					.iter()
					.for_each(|(_, hook)| hook.send(HookEvent::PeerAvailable(peer.clone())));
			}

			hooks.iter().for_each(|(_, hook)| {
				hook.send(HookEvent::PeerConnectedWith(listener, peer.clone()))
			});
		}

		peer
	}

	/// All active listeners registered with the P2P system.
	pub fn listeners(&self) -> Vec<Listener> {
		self.hooks
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.filter_map(|(id, hook)| {
				hook.listener.as_ref().map(|listener| Listener {
					id: ListenerId(id),
					name: hook.name,
					addrs: listener.addrs.clone(),
				})
			})
			.collect()
	}

	/// A listener is a special type of hook which is responsible for accepting incoming connections.
	///
	/// It is expected you call `Self::register_listener_addr` after this to register the addresses you are listening on.
	///
	/// `acceptor` is called when a peer is discovered, but before it is emitted to the application.
	/// This lets you register a connection method if you have one.
	pub fn register_listener(
		&self,
		name: &'static str,
		tx: Sender<HookEvent>,
		acceptor: impl Fn(ListenerId, &Arc<Peer>, &BTreeSet<PeerConnectionCandidate>)
			+ Send
			+ Sync
			+ 'static,
	) -> ListenerId {
		let mut hooks = self.hooks.write().unwrap_or_else(PoisonError::into_inner);
		let hook_id = hooks.push(Hook {
			name,
			tx,
			listener: Some(ListenerData {
				addrs: Default::default(),
				acceptor: HandlerFn(Arc::new(acceptor)),
			}),
		});

		hooks.iter().for_each(|(id, hook)| {
			if id == hook_id {
				return;
			}

			hook.send(HookEvent::ListenerRegistered(ListenerId(hook_id)));
		});

		ListenerId(hook_id)
	}

	pub fn register_listener_addr(&self, listener_id: ListenerId, addr: SocketAddr) {
		let mut hooks = self.hooks.write().unwrap_or_else(PoisonError::into_inner);
		if let Some(listener) = hooks
			.get_mut(listener_id.0)
			.and_then(|l| l.listener.as_mut())
		{
			listener.addrs.insert(addr);
		}

		info!("HookEvent::ListenerAddrAdded({listener_id:?}, {addr})");
		hooks.iter().for_each(|(_, hook)| {
			hook.send(HookEvent::ListenerAddrAdded(listener_id, addr));
		});
	}

	pub fn unregister_listener_addr(&self, listener_id: ListenerId, addr: SocketAddr) {
		let mut hooks = self.hooks.write().unwrap_or_else(PoisonError::into_inner);
		if let Some(listener) = hooks
			.get_mut(listener_id.0)
			.and_then(|l| l.listener.as_mut())
		{
			listener.addrs.remove(&addr);
		}

		info!("HookEvent::ListenerAddrRemoved({listener_id:?}, {addr})");
		hooks.iter().for_each(|(_, hook)| {
			hook.send(HookEvent::ListenerAddrRemoved(listener_id, addr));
		});
	}

	// TODO: Probs cleanup return type
	pub fn hooks(&self) -> Vec<(HookId, &'static str)> {
		self.hooks
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(id, hook)| (HookId(id), hook.name))
			.collect()
	}

	/// Register a new hook which can be used to react to state changes in the P2P system.
	pub fn register_hook(&self, name: &'static str, tx: Sender<HookEvent>) -> HookId {
		HookId(
			self.hooks
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.push(Hook {
					name,
					tx,
					listener: None,
				}),
		)
	}

	/// Unregister a hook. This will also call `HookEvent::Shutdown` on the hook.
	pub async fn unregister_hook(&self, id: HookId) {
		let mut shutdown_rxs = Vec::new();
		{
			let mut hooks = self.hooks.write().unwrap_or_else(PoisonError::into_inner);
			if let Some(hook) = hooks.remove(id.0) {
				let (_guard, rx) = ShutdownGuard::new();
				shutdown_rxs.push(rx);
				hook.send(HookEvent::Shutdown { _guard });

				if hook.listener.is_some() {
					hooks.iter().for_each(|(_, hook)| {
						hook.send(HookEvent::ListenerUnregistered(ListenerId(id.0)));
					});
				}

				let mut peers = self.peers.write().unwrap_or_else(PoisonError::into_inner);
				let mut peers_to_remove = HashSet::new(); // We are mutate while iterating
				for (identity, peer) in peers.iter_mut() {
					let mut state = peer.state.write().unwrap_or_else(PoisonError::into_inner);
					if state.active_connections.remove(&ListenerId(id.0)) {
						hooks.iter().for_each(|(_, hook)| {
							hook.send(HookEvent::PeerDisconnectedWith(
								ListenerId(id.0),
								peer.identity(),
							));
						});
					}
					state.connection_methods.remove(&ListenerId(id.0));
					state.discovered.remove(&id);

					hooks.iter().for_each(|(_, hook)| {
						hook.send(HookEvent::PeerExpiredBy(id, peer.identity()));
					});

					if state.connection_methods.is_empty() && state.discovered.is_empty() {
						peers_to_remove.insert(*identity);
					}
				}

				for identity in peers_to_remove {
					peers.remove(&identity);
				}
			}
		}

		// We rely on the fact that when the oneshot is dropped this will return an error as opposed to hanging.
		// So we can detect when the hooks shutdown code has completed.
		let _ = timeout(Duration::from_secs(2), join_all(shutdown_rxs)).await;
	}

	/// Shutdown the whole P2P system.
	/// This will close all connections and remove all hooks.
	pub async fn shutdown(&self) {
		let hooks = {
			self.hooks
				.write()
				.unwrap_or_else(PoisonError::into_inner)
				.iter()
				.map(|i| i.0)
				.collect::<Vec<_>>()
				.clone()
		};

		for hook_id in hooks {
			self.unregister_hook(HookId(hook_id)).await;
		}
	}
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Listener {
	pub id: ListenerId,
	pub name: &'static str,
	pub addrs: HashSet<SocketAddr>,
}

impl Listener {
	pub fn is_hook_id(&self, id: HookId) -> bool {
		self.id.0 == id.0
	}
}
