//! Rust Peer to Peer Networking Library
#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]

mod identity;
mod mdns;
mod p2p;
mod peer;
pub mod quic;
mod stream;

pub use identity::{Identity, IdentityErr, RemoteIdentity, REMOTE_IDENTITY_LEN};
pub use mdns::Mdns;
pub use p2p::{HookEvent, HookId, P2P};
pub use peer::{Peer, PeerStatus};
pub use stream::UnicastStream;
