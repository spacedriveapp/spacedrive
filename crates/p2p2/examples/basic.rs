use std::{
	env,
	net::{Ipv4Addr, SocketAddrV4},
	time::Duration,
};

use p2p2::{Endpoint, Identity, LogTransport, QuicTransport};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

pub struct PeerMetadata {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Intent {
	Spacedrop,
	Pairing,
	Sync,
}

#[tokio::main]
async fn main() {
	let identity = Identity::new().unwrap();

	let transport = QuicTransport::new(
		identity.clone(),
		SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
	)
	.await;
	let transport = LogTransport::new(transport); // We can build wrapper transports for encryption, etc

	let endpoint = Endpoint::new(transport, &identity, |msg: Intent, stream| async move {
		// TODO: These should be `conn.event().await` or something for listening for disconnect events??? -> The end user is gonna need to be responsible for closing connections

		match msg {
			Intent::Spacedrop => {
				println!("Spacedrop");
			}
			Intent::Pairing => {
				println!("Pairing");
			}
			Intent::Sync => {
				println!("Sync");
			}
		}
	})
	.unwrap();

	println!("Listening {:?}", endpoint.listen_addr());

	match env::var("P2POG_MODE").unwrap_or_default().as_str() {
		"server" => {}
		_ => {
			let conn = endpoint
				.connect(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 60365).into())
				.await
				.unwrap();
			println!("Established connection");
			let stream = conn.stream(Intent::Spacedrop).await.unwrap();
			println!("Established stream");

			sleep(Duration::from_secs(100000)).await; // TODO: Remove this and handle drop semantics on a sending stream
		}
	}

	// stream.write("Hello, world!").await.unwrap();

	// TODO: When `conn` is dropped it should auto close the connection.
	// conn.close(); // or

	// TODO: Drop all connections with going offline specific error when `Endpoint` is dropped.

	// let y = endpoint.state().protector.ban_peer();

	// let srv = Service::new("spacedrive", move || PeerMetadata {});

	// let x = TracedConnections::new(|peer_id: PeerId, metadata: PeerMetadata /* TODO: Include information of "Service" which discovered/it is contacting it */| {
	//     return true; // Should peer be connected?
	// });  // TODO: Endpoint should provide primitives like `connect()`, `disconnect()`, etc

	// // TODO: Subscribe to discover, expire, connect, disconnect events

	// // endpoint.connection_peer();
	// // endpoint.ban_peer();
	// // endpoint.stats();
	// // endpoint.peer_id();

	sleep(Duration::from_secs(100000)).await;
}
