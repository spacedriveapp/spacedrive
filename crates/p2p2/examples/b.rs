use std::net::{Ipv4Addr, SocketAddr};

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

	let (quic, libp2p_peer_id) = QuicTransport::spawn(p2p.clone(), 8076).unwrap();
	println!("{:?}", libp2p_peer_id);

	// Enable IPv4 (`Some`) on a random port (`0`)
	// quic.set_ipv4_enabled(Some(08076)).await.unwrap();
	// quic.set_ipv6_enabled(Some(0)).await.unwrap();

	let (hook_tx, hook_rx) = flume::bounded(69);
	let hook_id = p2p.register_hook("p2p-mock-discovery", hook_tx);

	let mut sleep = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(1)));
	loop {
		tokio::select! {
			event = handler_rx.recv_async() => {
				println!("HANDLER: {:?}", event.unwrap());
			}
			event = hook_rx.recv_async() => {
				println!("HOOK: {:?}", event.unwrap());
			}
			_ = &mut sleep => {
				sleep = Box::pin(tokio::time::sleep(tokio::time::Duration::from_secs(10000000)));
				println!("DIAL");
				let peer = p2p.clone().discover_peer(hook_id, Identity::new().to_remote_identity(), Default::default(), [SocketAddr::from((
						Ipv4Addr::LOCALHOST,
						8075,
					))].into_iter().collect());

				let stream = peer.new_stream().await.unwrap();
				panic!("DID THE THING");
			}
		}
	}
}
