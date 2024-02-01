use std::{
	borrow::Cow,
	collections::HashMap,
	net::SocketAddr,
	sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use stable_vec::StableVec;
use tokio::sync::mpsc;

use crate::{Identity, Peer, RemoteIdentity};

// TODO: `HookEvent` split into `Add`/`Delete` operation???
// TODO: Fire `HookEvent`'s on change using custom `MutexGuard`
// TODO: Finish shutdown process
// TODO: Rename `discovered` property cause it's more than that

// TODO: mDNS service removal fully hooked up

#[derive(Debug, Clone)]
pub enum HookEvent {
	/// A change to `P2P::service`
	MetadataChange(String),
	/// A change to `P2P::discovered`
	DiscoveredChange(RemoteIdentity),
	/// A change to `P2P::listeners`
	ListenersChange(String),
	/// Shutting down the P2P manager
	Shutdown,
}

#[derive(Debug, Clone)]
pub struct HookId(usize);

type ListenerName = Cow<'static, str>;

/// Manager for the entire P2P system.
#[derive(Debug)]
pub struct P2P {
	/// A unique identifier for the application.
	/// This will differentiate between different applications using this same P2P library.
	app_name: &'static str,
	/// The identity of the local node.
	/// This is the public/private keypair used to uniquely identify the node.
	identity: Identity,
	/// A metadata being shared from the local node to the remote nodes.
	/// This will contain information such as the node's name, version, and services we provide.
	metadata: RwLock<HashMap<String, String>>,
	/// A list of all discovered nodes and their metadata (which comes from `self.service` above).
	discovered: RwLock<HashMap<RemoteIdentity, Peer>>,
	/// A list of active listeners on the current node.
	listeners: RwLock<HashMap<ListenerName, SocketAddr>>,
	/// Hooks can be registered to react to state changes.
	hooks: Mutex<StableVec<mpsc::Sender<HookEvent>>>,
}

impl P2P {
	pub fn new(app_name: &'static str, identity: Identity) -> Arc<Self> {
		// TODO: Validate `app_name` is valid for mDNS

		Arc::new(P2P {
			app_name,
			identity,
			metadata: Default::default(),
			discovered: Default::default(),
			listeners: Default::default(),
			hooks: Default::default(),
		})
	}

	pub fn app_name(&self) -> &'static str {
		self.app_name
	}

	pub fn identity(&self) -> &Identity {
		&self.identity
	}

	pub fn remote_identity(&self) -> RemoteIdentity {
		self.identity.to_remote_identity()
	}

	pub fn metadata(&self) -> RwLockReadGuard<HashMap<String, String>> {
		self.metadata.read().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn metadata_mut(&self) -> RwLockWriteGuard<HashMap<String, String>> {
		self.metadata
			.write()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn discovered(&self) -> RwLockReadGuard<HashMap<RemoteIdentity, Peer>> {
		self.discovered
			.read()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn discovered_mut(&self) -> RwLockWriteGuard<HashMap<RemoteIdentity, Peer>> {
		// TODO: before releasing the lock we should ask the hooks if they wanna register a connection method

		self.discovered
			.write()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn listeners(&self) -> RwLockReadGuard<HashMap<ListenerName, SocketAddr>> {
		self.listeners
			.read()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn listeners_mut(&self) -> RwLockWriteGuard<HashMap<ListenerName, SocketAddr>> {
		self.listeners
			.write()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn register_hook(&self, tx: mpsc::Sender<HookEvent>) -> HookId {
		HookId(
			self.hooks
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.push(tx),
		)
	}

	pub fn unregister_hook(&self, id: HookId) {
		if let Some(sender) = self
			.hooks
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(id.0)
		{
			let _ = sender.send(HookEvent::Shutdown);
		}
	}

	pub fn shutdown(&self) {
		let hooks = self.hooks.lock().unwrap_or_else(PoisonError::into_inner);
		for (_, tx) in hooks.iter() {
			let _ = tx.send(HookEvent::Shutdown);
		}

		// TODO: Wait for response from hooks saying they are done shutting down
		// TODO: Maybe wait until `unregister_hook` is called internally by each of them or with timeout and force removal overwise.
	}
}
