use std::{
	future::Future,
	net::{IpAddr, SocketAddr},
};

use libp2p::{multiaddr::Protocol, Multiaddr};

// TODO: Turn these into From/Into impls on a wrapper type

pub(crate) fn quic_multiaddr_to_socketaddr(m: Multiaddr) -> Result<SocketAddr, String> {
	let mut addr_parts = m.iter();

	let addr = match addr_parts.next() {
		Some(Protocol::Ip4(addr)) => IpAddr::V4(addr),
		Some(Protocol::Ip6(addr)) => IpAddr::V6(addr),
		Some(proto) => {
			return Err(format!(
				"Invalid multiaddr. Segment 1 found protocol 'Ip4' or 'Ip6' but found  '{}'",
				proto
			))
		}
		None => return Err(format!("Invalid multiaddr. Segment 1 missing")),
	};

	let port = match addr_parts.next() {
		Some(Protocol::Udp(port)) => port,
		Some(proto) => {
			return Err(format!(
				"Invalid multiaddr. Segment 2 expected protocol 'Udp' but found  '{}'",
				proto
			))
		}
		None => return Err(format!("Invalid multiaddr. Segment 2 missing")),
	};

	Ok(SocketAddr::new(addr, port))
}

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

// A trait which allows me to represent a closure, it's future return type and the futures return type with only a single generic.
pub trait AsyncFn
where
	Self: Fn() -> Self::Future + Send + Sync + 'static,
{
	type Output;
	type Future: Future<Output = <Self as AsyncFn>::Output> + Send;
}

impl<TOutput, TFut, TFunc> AsyncFn for TFunc
where
	TFut: Future<Output = TOutput> + Send,
	TFunc: Fn() -> TFut + Send + Sync + 'static,
{
	type Output = TOutput;
	type Future = TFut;
}

// A trait which allows me to represent a closure, it's future return type and the futures return type with only a single generic.
pub trait AsyncFn2<Arg, Arg2>
where
	Self: Fn(Arg, Arg2) -> Self::Future + Send + Sync + 'static,
{
	type Output;
	type Future: Future<Output = <Self as AsyncFn2<Arg, Arg2>>::Output> + Send;
}

impl<TArg, Arg2, TOutput, TFut, TFunc> AsyncFn2<TArg, Arg2> for TFunc
where
	TFut: Future<Output = TOutput> + Send,
	TFunc: Fn(TArg, Arg2) -> TFut + Send + Sync + 'static,
{
	type Output = TOutput;
	type Future = TFut;
}
