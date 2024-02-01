//! Rust Peer to Peer Networking Library
#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]

mod event;
mod manager;
mod manager_stream;
mod peer;
pub mod spacetime;
pub mod spacetunnel;
mod utils;

pub use discovery::*;
pub use event::*;
pub use manager::*;
pub use manager_stream::*;
pub use peer::*;
pub use utils::*;

// TODO: Remove this
#[doc(hidden)]
pub mod internal {
	pub use libp2p::PeerId;
}
