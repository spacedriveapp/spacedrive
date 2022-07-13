mod discovery;
mod network_manager;
mod p2p_manager;
mod peer;
mod utils;

pub use discovery::*;
pub use network_manager::*;
pub use p2p_manager::*;
pub use peer::*;
pub use sd_tunnel_utils::PeerId;
pub use utils::*;

/// We reexport some types from `quinn` to avoid the user needing to add `quinn` and keep its version in sync with the p2p library.
pub mod quinn {
	pub use quinn::{RecvStream, SendStream};
}
