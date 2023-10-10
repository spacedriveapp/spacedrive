use std::{collections::HashMap, env, time::Duration};

use sd_p2p::{Event, Keypair, Manager, Metadata, MetadataManager, PeerId};
use tokio::{io::AsyncReadExt, time::sleep};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct PeerMetadata {
	name: String,
}

impl Metadata for PeerMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		HashMap::from([("name".to_owned(), self.name)])
	}

	fn from_hashmap(_: &PeerId, data: &HashMap<String, String>) -> Result<Self, String>
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
				.add_directive("basic=trace".parse().unwrap())
				.add_directive("sd-p2p=trace".parse().unwrap())
				.add_directive("info".parse().unwrap()),
		)
		.try_init()
		.unwrap();

	let keypair = Keypair::generate();

	let metadata_manager = MetadataManager::new(PeerMetadata {
		name: "TODO".to_string(),
	});

	let (manager, mut stream) = Manager::new("p2p-demo", &keypair, metadata_manager)
		.await
		.unwrap();

	info!(
		"Node '{}' is now online listening at addresses: {:?}",
		manager.peer_id(),
		manager.listen_addrs().await
	);

	tokio::spawn(async move {
		let mut shutdown = false;
		// Your application must keeping poll this stream to keep the P2P system running
		while let Some(event) = stream.next().await {
			match event {
				Event::PeerDiscovered(event) => {
					println!(
						"Discovered peer by id '{}' with address '{:?}' and metadata: {:?}",
						event.peer_id, event.addresses, event.metadata
					);
					event.dial().await; // We connect to everyone we find on the network. Your app will probs wanna restrict this!
				}
				Event::PeerMessage(mut event) => {
					debug!("Peer '{}' established unicast stream", event.peer_id);

					tokio::spawn(async move {
						let mut buf = [0; 100];
						let n = event.stream.read(&mut buf).await.unwrap();
						println!("GOT UNICAST: {:?}", std::str::from_utf8(&buf[..n]).unwrap());
					});
				}
				Event::PeerBroadcast(mut event) => {
					debug!("Peer '{}' established broadcast stream", event.peer_id);

					tokio::spawn(async move {
						let mut buf = [0; 100];
						let n = event.stream.read(&mut buf).await.unwrap();
						println!(
							"GOT BROADCAST: {:?}",
							std::str::from_utf8(&buf[..n]).unwrap()
						);
					});
				}
				Event::Shutdown => {
					info!("Manager shutdown!");
					shutdown = true;
					break;
				}
				_ => debug!("event: {:?}", event),
			}
		}

		if !shutdown {
			error!("Manager event stream closed! The core is unstable from this point forward!");
			// process.exit(1); // TODO: Should I?
		}
	});

	if env::var("PING").as_deref() != Ok("skip") {
		let manager = manager.clone();
		tokio::spawn(async move {
			sleep(Duration::from_millis(500)).await;

			// Send pings to every client every 3 second after startup
			loop {
				sleep(Duration::from_secs(3)).await;
				manager
					.broadcast(
						format!("Hello World From {}", keypair.peer_id())
							.as_bytes()
							.to_vec(),
					)
					.await;
				debug!("Sent ping broadcast to all connected peers!");
			}
		});
	}

	// TODO: proper shutdown
	// https://docs.rs/ctrlc/latest/ctrlc/
	// https://docs.rs/system_shutdown/latest/system_shutdown/

	tokio::time::sleep(Duration::from_secs(100)).await;

	manager.shutdown().await; // It is super highly recommended to shutdown the manager before exiting your application so an Mdns update can be broadcasted
}
