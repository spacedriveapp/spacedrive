//! A system for creating encrypted tunnels between peers over untrusted connections.

mod tunnel;

pub use sd_p2p2::{Identity, IdentityErr, RemoteIdentity, REMOTE_IDENTITY_LEN};
pub use tunnel::*;
