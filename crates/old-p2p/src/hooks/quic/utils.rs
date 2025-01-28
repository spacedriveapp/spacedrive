//! This file contains some fairly meaningless glue code for integrating with libp2p.

use std::net::SocketAddr;

use libp2p::{identity::Keypair, multiaddr::Protocol, Multiaddr, PeerId};

use crate::{Identity, RemoteIdentity};

#[must_use]
pub(crate) fn socketaddr_to_multiaddr(m: &SocketAddr) -> Multiaddr {
	let mut addr = Multiaddr::empty();
	match m {
		SocketAddr::V4(ip) => addr.push(Protocol::Ip4(*ip.ip())),
		SocketAddr::V6(ip) => addr.push(Protocol::Ip6(*ip.ip())),
	}
	addr.push(Protocol::Udp(m.port()));
	addr.push(Protocol::QuicV1);
	addr
}

#[must_use]
pub(crate) fn multiaddr_to_socketaddr(m: &Multiaddr) -> Option<SocketAddr> {
	let mut iter = m.iter();
	let ip = match iter.next()? {
		Protocol::Ip4(ip) => ip.into(),
		Protocol::Ip6(ip) => ip.into(),
		_ => return None,
	};
	let port = match iter.next()? {
		Protocol::Tcp(port) | Protocol::Udp(port) => port,
		_ => return None,
	};
	Some(SocketAddr::new(ip, port))
}

// This is sketchy, but it makes the whole system a lot easier to work with
// We are assuming the libp2p `PublicKey` is the same format as our `RemoteIdentity` type.
// This is *acktually* true but they reserve the right to change it at any point.
#[must_use]
pub fn remote_identity_to_libp2p_peerid(identity: &RemoteIdentity) -> PeerId {
	let public_key = libp2p::identity::ed25519::PublicKey::try_from_bytes(&identity.get_bytes())
		.expect("should be the same format");
	PeerId::from_public_key(&public_key.into())
}

// This is sketchy, but it makes the whole system a lot easier to work with
// We are assuming the libp2p `Keypair` is the same format as our `Identity` type.
// This is *acktually* true but they reserve the right to change it at any point.
#[must_use]
pub fn identity_to_libp2p_keypair(identity: &Identity) -> Keypair {
	libp2p::identity::Keypair::ed25519_from_bytes(identity.to_bytes())
		.expect("should be the same format")
}
