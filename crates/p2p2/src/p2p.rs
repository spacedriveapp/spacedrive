use std::{
	borrow::Cow,
	collections::HashMap,
	fmt,
	hash::{Hash, Hasher},
	net::SocketAddr,
	ops::{Deref, DerefMut},
	sync::{Arc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use stable_vec::StableVec;
use tokio::sync::mpsc;

use crate::{Identity, Peer, RemoteIdentity, UnicastStream};

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

/// A little wrapper for functions to make them `Debug`.
#[derive(Clone)]
struct HandlerFn<F>(F);

impl<F> fmt::Debug for HandlerFn<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "HandlerFn")
	}
}

#[derive(Debug, Clone)]
pub struct Listener {
	addr: SocketAddr,
	acceptor: HandlerFn<Arc<dyn Fn(&mut Peer, &Vec<SocketAddr>) + Send + Sync>>,
}

impl Hash for Listener {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.addr.hash(state);

		let acceptor_ptr: *const _ = &*self.acceptor.0;
		let acceptor_ptr = acceptor_ptr as *const () as usize;
		acceptor_ptr.hash(state);
	}
}

impl Listener {
	pub fn new(
		addr: SocketAddr,
		acceptor: impl Fn(&mut Peer, &Vec<SocketAddr>) + Send + Sync + 'static,
	) -> Arc<Self> {
		Arc::new(Self {
			addr,
			acceptor: HandlerFn(Arc::new(acceptor)),
		})
	}

	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	pub fn acceptor(&self, peer: &mut Peer, addrs: &Vec<SocketAddr>) {
		(self.acceptor.0)(peer, addrs);
	}
}

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
	/// Each listener have an acceptor function is called by discovery when a new peer is found prior to it being emitted to the application.
	listeners: RwLock<HashMap<ListenerName, Listener>>,
	/// The function used to accept incoming connections.
	handler: HandlerFn<Box<dyn Fn(UnicastStream) + Send + Sync>>,
	/// Hooks can be registered to react to state changes.
	hooks: Mutex<StableVec<mpsc::Sender<HookEvent>>>,
}

impl P2P {
	pub fn new(
		app_name: &'static str,
		identity: Identity,
		handler: impl Fn(UnicastStream) + Send + Sync + 'static,
	) -> Arc<Self> {
		// TODO: Validate `app_name`'s max length too
		// app_name
		// 	.chars()
		// 	.all(|c| char::is_alphanumeric(c) || c == '-')
		// 	.then_some(())
		// 	.ok_or(ManagerError::InvalidAppName)?;

		Arc::new(P2P {
			app_name,
			identity,
			metadata: Default::default(),
			discovered: Default::default(),
			listeners: Default::default(),
			handler: HandlerFn(Box::new(handler)),
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

	pub fn metadata_mut(&self) -> SmartWriteGuard<HashMap<String, String>> {
		let lock = self
			.metadata
			.write()
			.unwrap_or_else(PoisonError::into_inner);

		SmartWriteGuard {
			p2p: self,
			before: Some(lock.clone()),
			lock,
			save: |p2p, before, after| {
				// TODO

				// let hooks = p2p.hooks.lock().unwrap_or_else(PoisonError::into_inner);
				// for (_, tx) in hooks.iter() {
				// 	let _ = tx.send(HookEvent::MetadataChange("".into()));
				// }
			},
		}
	}

	pub fn discovered(&self) -> RwLockReadGuard<HashMap<RemoteIdentity, Peer>> {
		self.discovered
			.read()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn discovered_mut(&self) -> SmartWriteGuard<HashMap<RemoteIdentity, Peer>> {
		let lock = self
			.discovered
			.write()
			.unwrap_or_else(PoisonError::into_inner);

		SmartWriteGuard {
			p2p: self,
			before: Some(lock.clone()),
			lock,
			save: |p2p, before, after| {
				// TODO: before releasing the lock we should ask the `listeners` if they wanna register a connection method

				// let hooks = p2p.hooks.lock().unwrap_or_else(PoisonError::into_inner);
				// for (_, tx) in hooks.iter() {
				// 	let _ = tx.send(HookEvent::MetadataChange("".into()));
				// }
			},
		}
	}

	pub fn listeners(&self) -> RwLockReadGuard<HashMap<ListenerName, Listener>> {
		self.listeners
			.read()
			.unwrap_or_else(PoisonError::into_inner)
	}

	pub fn listeners_mut(&self) -> SmartWriteGuard<HashMap<ListenerName, Listener>> {
		let lock = self
			.listeners
			.write()
			.unwrap_or_else(PoisonError::into_inner);

		SmartWriteGuard {
			p2p: self,
			before: Some(lock.clone()),
			lock,
			save: |p2p, before, after| {
				// TODO

				// let hooks = p2p.hooks.lock().unwrap_or_else(PoisonError::into_inner);
				// for (_, tx) in hooks.iter() {
				// 	let _ = tx.send(HookEvent::MetadataChange("".into()));
				// }
			},
		}
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

/// A smart guard for `RwLock` that will call a save function when it's dropped.
/// This allows changes to the value to automatically trigger `HookEvents` to be emitted.
#[derive(Debug)]
pub struct SmartWriteGuard<'a, T> {
	p2p: &'a P2P,
	lock: RwLockWriteGuard<'a, T>,
	before: Option<T>,
	save: fn(&P2P, /* before */ T, /* after */ &T),
}

impl<'a, T> Deref for SmartWriteGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock
	}
}

impl<'a, T> DerefMut for SmartWriteGuard<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lock
	}
}

impl<'a, T> Drop for SmartWriteGuard<'a, T> {
	fn drop(&mut self) {
		(self.save)(
			self.p2p,
			self.before
				.take()
				.expect("'SmartWriteGuard::drop' called more than once!"),
			&self.lock,
		);
	}
}
