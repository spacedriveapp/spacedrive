//! `Spacetime` is just a fancy name for the protocol which sits between libp2p and the application built on this library.
//! This protocol sits under the application to abstract many complexities of 2 way connections and deals with authentication, chucking, etc.

mod behaviour;
mod connection;
mod libp2p;
mod proto_inbound;
mod proto_outbound;
mod stream;

pub use self::libp2p::*;
pub use behaviour::*;
pub use connection::*;
pub use proto_inbound::*;
pub use proto_outbound::*;
pub use stream::*;
