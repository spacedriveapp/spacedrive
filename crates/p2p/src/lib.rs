//! Rust Peer to Peer Networking Library
#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]

pub(crate) mod hooks;
mod identity;
mod mdns;
mod p2p;
mod peer;
mod quic;
mod smart_guards;
mod stream;

pub use hooks::{HookEvent, HookId, ListenerId, ShutdownGuard};
pub use identity::{Identity, IdentityErr, RemoteIdentity};
pub use mdns::Mdns;
pub use p2p::{Listener, P2P};
pub use peer::{ConnectionRequest, Peer, PeerConnectionCandidate};
pub use quic::{Libp2pPeerId, QuicTransport, RelayServerEntry};
pub use smart_guards::SmartWriteGuard;
pub use stream::UnicastStream;

pub use flume;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Error)]
pub enum NewStreamError {
	#[error("No connection methods available for peer")]
	NoConnectionMethodsAvailable,
	#[error("The event loop is offline")]
	EventLoopOffline(mpsc::error::SendError<ConnectionRequest>),
	#[error("Failed to establish the connection w/ error: {0}")]
	ConnectionNeverEstablished(oneshot::error::RecvError),
	#[error("error connecting to peer: {0}")]
	Connecting(String),
}
