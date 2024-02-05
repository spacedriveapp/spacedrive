use std::{collections::HashSet, fmt, net::SocketAddr, sync::Arc};

use tokio::sync::mpsc;

use crate::{Peer, RemoteIdentity};

#[derive(Debug, Clone)]
pub enum HookEvent {
	/// `P2P::service` has changed
	MetadataModified,

	/// A new listener was registered with the P2P system.
	ListenerRegistered { id: ListenerId, addr: SocketAddr },
	/// A listener was unregistered from the P2P system.
	ListenerUnregistered(ListenerId),

	/// A peer was inserted into `P2P::peers`
	/// This peer could have connected to or have been discovered by a hook.
	PeerAvailable(Arc<Peer>),
	/// A peer was removed from `P2P::peers`
	/// This is due to it no longer being discovered, containing no active connections or available connection methods.
	PeerUnavailable(RemoteIdentity),

	/// A peer was discovered by a hook
	/// This will fire for *every peer* per every *hook* that discovers it.
	PeerDiscoveredBy(HookId, Arc<Peer>),
	/// A hook expired a peer
	/// This will fire for *every peer* per every *hook* that discovers it.
	PeerExpiredBy(HookId, RemoteIdentity),

	/// "Connections" are an internal concept to the P2P library but they will be automatically triggered by `Peer::new_stream`.
	/// They are a concept users of the application may care about so they are exposed here.

	/// A new listener established a connection with a peer
	PeerConnectedWith(ListenerId, Arc<Peer>),
	/// A connection closed with a peer.
	PeerDisconnectedWith(ListenerId, RemoteIdentity),

	/// Your hook or the P2P system was told to shutdown.
	Shutdown,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct HookId(pub(crate) usize);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ListenerId(pub(crate) usize);

impl From<ListenerId> for HookId {
	fn from(value: ListenerId) -> Self {
		Self(value.0)
	}
}

#[derive(Debug)]
pub(crate) struct Hook {
	/// A name used for debugging purposes.
	pub(crate) name: &'static str,
	/// A channel to send events to the hook.
	/// This hooks implementing will be responsible for subscribing to this channel.
	pub(crate) tx: mpsc::Sender<HookEvent>,
	/// If this hook is a listener this will be set.
	pub(crate) listener: Option<ListenerData>,
}

impl Hook {
	pub fn send(&self, event: HookEvent) {
		let _ = self.tx.send(event);
	}

	pub fn acceptor(&self, peer: &Arc<Peer>, addrs: &Vec<SocketAddr>) {
		if let Some(listener) = &self.listener {
			(listener.acceptor.0)(peer, addrs);
		}
	}
}

#[derive(Debug)]
pub(crate) struct ListenerData {
	/// The address the listener is bound to.
	/// These will be advertised by any discovery methods attached to the P2P system.
	pub addrs: HashSet<SocketAddr>,
	/// This is a function over a channel because we need to ensure the code runs prior to the peer being emitted to the application.
	/// If not the peer would have no registered way to connect to it initially which would be confusing.
	pub acceptor: HandlerFn<Arc<dyn Fn(&Arc<Peer>, &Vec<SocketAddr>) + Send + Sync>>,
}

/// A little wrapper for functions to make them `Debug`.
#[derive(Clone)]
pub(crate) struct HandlerFn<F>(pub(crate) F);

impl<F> fmt::Debug for HandlerFn<F> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "HandlerFn")
	}
}
