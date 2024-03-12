//! This file contains some fairly meaningless glue code for integrating with libp2p.

use std::net::SocketAddr;

use libp2p::{multiaddr::Protocol, Multiaddr};

#[must_use]
pub(crate) fn socketaddr_to_quic_multiaddr(m: &SocketAddr) -> Multiaddr {
	let mut addr = Multiaddr::empty();
	match m {
		SocketAddr::V4(ip) => addr.push(Protocol::Ip4(*ip.ip())),
		SocketAddr::V6(ip) => addr.push(Protocol::Ip6(*ip.ip())),
	}
	addr.push(Protocol::Udp(m.port()));
	addr.push(Protocol::QuicV1);
	addr
}
