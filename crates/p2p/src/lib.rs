//! Rust Peer to Peer Networking Library

mod discovery;
mod event;
mod manager;
mod manager_stream;
mod peer;
pub mod proto;
pub mod spaceblock;
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
