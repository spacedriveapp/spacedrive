//! Quic-based transport.
//!
//! This uses [libp2p](https://docs.rs/libp2p) under the hood.

pub(super) mod handle;
pub(super) mod transport;
pub(super) mod utils;

pub use handle::QuicHandle;
pub use transport::{Libp2pPeerId, QuicTransport, RelayServerEntry};
