use std::{collections::HashMap, time::Duration};

use sd_p2p::{Event, Keypair, Manager, Metadata};
use tokio::time::sleep;
use tracing::info;

#[derive(Debug, Clone)]
pub struct PeerMetadata {
	name: String,
}

impl Metadata for PeerMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		HashMap::from([("name".to_owned(), self.name)])
	}

	fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized,
	{
		Ok(Self {
			name: data
				.get("name")
				.ok_or_else(|| {
					"DNS record for field 'name' missing. Unable to decode 'PeerMetadata'!"
						.to_owned()
				})?
				.to_owned(),
		})
	}
}

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::from_default_env()
				.add_directive("sd_p2p=trace".parse().unwrap())
				.add_directive("info".parse().unwrap()),
		)
		.try_init()
		.unwrap();

	let keypair = Keypair::generate();

	let manager = Manager::new(
		"p2p3-demo",
		&keypair,
		|| async move {
			PeerMetadata {
				name: "TODO".to_string(),
			}
		},
		|manager, event| async move {
			match event {
				Event::PeerDiscovered(event) => {
					println!(
						"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
						event.peer_id(),
						event.addresses(),
						event.metadata()
					);
					event.dial(&manager).await; // We connect to everyone we find on the network. Your app will probs wanna restrict this!
				}
				event => println!("{:?}", event),
			}
		},
		// This closure it run to handle a single incoming request. It's return type is then sent back to the client.
		// TODO: Why can't it infer the second param here???
		|_manager, data: Vec<u8>| async move {
			println!(
				"Received message: {:?}",
				std::str::from_utf8(&data).unwrap()
			);

			Ok(data) // We echo the request back
		},
	)
	.await
	.unwrap();

	tokio::spawn(async move {
		sleep(Duration::from_millis(500)).await;
		info!(
			"Node '{}' is now online listening at addresses: {:?}",
			manager.peer_id(),
			manager.listen_addrs().await
		);

		// Send pings to every client every 3 second after startup
		loop {
			sleep(Duration::from_secs(3)).await;
			manager
				.clone()
				.broadcast(format!("Hello World From {}", keypair.public().to_peer_id()).as_bytes())
				.await
				.unwrap();
			println!("Sent broadcast!");
		}
	});

	// TODO: proper shutdown
	// https://docs.rs/ctrlc/latest/ctrlc/
	// https://docs.rs/system_shutdown/latest/system_shutdown/

	tokio::time::sleep(Duration::from_secs(100)).await;
}
