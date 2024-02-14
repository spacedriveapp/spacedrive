use sd_p2p2::{Identity, QuicTransport, P2P};

#[tokio::main]
async fn main() {
	std::env::set_var("RUST_LOG", "trace");
	tracing_subscriber::fmt::init();

	let identity = Identity::new();
	println!("{:?}", identity);

	let (handler_tx, handler_rx) = flume::bounded(69);
	let p2p = P2P::new("bruh", identity, handler_tx);

	// TODO: Mount mdns

	let (quic, libp2p_peer_id) = QuicTransport::spawn(p2p, 8075).unwrap();
	println!("{:?}", libp2p_peer_id);

	// Enable IPv4 (`Some`) on a random port (`0`)
	// quic.set_ipv4_enabled(Some(8075)).await.unwrap();
	// quic.set_ipv6_enabled(Some(0)).await.unwrap();

	loop {
		let event = handler_rx.recv_async().await.unwrap();
		println!("{:?}", event);
	}
}
