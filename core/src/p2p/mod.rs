#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]
#![allow(clippy::unnecessary_cast)] // Yeah they aren't necessary on this arch, but they are on others

use sd_p2p2::{Identity, P2P};

mod identity_or_remote_identity;
mod libraries;
mod library_metadata;
pub mod operations;
mod p2p_events;
mod p2p_manager;
mod p2p_manager_actor;
mod peer_metadata;
mod protocol;
mod state;
pub mod sync;

pub use identity_or_remote_identity::*;
pub use libraries::*;
pub use library_metadata::*;
pub use p2p_events::*;
pub use p2p_manager::*;
pub use p2p_manager_actor::*;
pub use peer_metadata::*;
pub use protocol::*;

pub use state::*;

pub(super) const SPACEDRIVE_APP_ID: &str = "sd";

// TODO: How to enabled/disable mDNS
// TODO: How to enable/disable the libp2p connection layer
fn todo_v2() {
	let identity = Identity::new();
	let p2p = P2P::new(SPACEDRIVE_APP_ID, identity);

	// TODO: Hook up mdns

	// TODO: Hook up P2P

	// TODO: Store state for each library somehow? How's offline, discovered or connected

	// TODO: Subscribe to changes in discovered nodes & connect

	p2p.shutdown();
}
