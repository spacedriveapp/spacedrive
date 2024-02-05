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

pub use hooks::{HookEvent, HookId, ListenerId};
pub use identity::{Identity, IdentityErr, RemoteIdentity};
pub use mdns::Mdns;
pub use p2p::{Listener, P2P};
pub use peer::Peer;
pub use quic::{Libp2pPeerId, QuicTransport};
pub use smart_guards::SmartWriteGuard;
pub use stream::UnicastStream;
