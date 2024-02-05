mod behaviour;
mod connection;
mod libp2p;
mod proto_inbound;
mod proto_outbound;
mod stream;
mod transport;

pub use transport::{Libp2pPeerId, QuicTransport};

pub(super) use libp2p::SpaceTimeProtocolName;
