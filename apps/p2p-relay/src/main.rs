use std::{
	io::{stdin, stdout, Write},
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	path::PathBuf,
};

use libp2p::{
	autonat,
	futures::StreamExt,
	relay,
	swarm::{NetworkBehaviour, SwarmEvent},
};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::utils::socketaddr_to_quic_multiaddr;

mod config;
mod utils;

// TODO: Authentication with the Spacedrive Cloud
// TODO: Rate-limit data usage by Spacedrive account.
// TODO: Expose libp2p metrics like - https://github.com/mxinden/rust-libp2p-server/blob/master/src/behaviour.rs

#[derive(NetworkBehaviour)]
pub struct Behaviour {
	relay: relay::Behaviour,
	autonat: autonat::Behaviour,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelayServerEntry {
	id: Uuid,
	// TODO: Try and drop this field cause it's libp2p specific
	peer_id: String,
	addrs: Vec<SocketAddr>,
}

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt()
		// .with_env_filter(EnvFilter::from_default_env()) // TODO: ???
		.init();

	let config_path =
		PathBuf::from(std::env::var("CONFIG_PATH").unwrap_or("./config.json".to_string()));

	let mut args = std::env::args();
	args.next(); // Skip binary name
	if args.next().as_deref() == Some("init") {
		println!("Initializing config at '{config_path:?}'...");

		if config_path.exists() {
			panic!("Config already exists at path '{config_path:?}'. Please delete it first!");
			// TODO: Error handling
		}

		print!("Please enter the p2p secret: ");
		let mut p2p_secret = String::new();
		let _ = stdout().flush();
		stdin()
			.read_line(&mut p2p_secret)
			.expect("Did not enter a correct string");

		config::Config::init(&config_path, p2p_secret.replace('\n', "")).unwrap(); // TODO: Error handling
		println!("\nSuccessfully initialized config at '{config_path:?}'!");
		return;
	}

	if !config_path.exists() {
		panic!("Unable to find config at path '{config_path:?}'. Please create it!"); // TODO: Error handling
	}
	let config = config::Config::load(&config_path).unwrap(); // TODO: Error handling

	info!("Starting...");

	let public_ipv4: Ipv4Addr = reqwest::get("https://api.ipify.org")
		.await
		.unwrap() // TODO: Error handling
		.text()
		.await
		.unwrap() // TODO: Error handling
		.parse()
		.unwrap(); // TODO: Error handling

	let public_ipv6: Option<Ipv6Addr> = match reqwest::get("https://api6.ipify.org").await {
		Ok(v) => Some(
			v.text()
				.await
				.unwrap() // TODO: Error handling
				.parse()
				.unwrap(), // TODO: Error handling
		),
		Err(_) => {
			warn!("Error getting public IPv6 address. Skipping IPv6 configuration.");
			None
		}
	};

	info!("Determined public addresses of the current relay to be: '{public_ipv4}' and '{public_ipv6:?}'");

	let (first_advertisement_tx, mut first_advertisement_rx) = tokio::sync::mpsc::channel(1);
	tokio::spawn({
		let config = config.clone();
		async move {
			let client = reqwest::Client::new();

			let mut first_advertisement_tx = Some(first_advertisement_tx);
			loop {
				let result = client
					.post(format!("{}/api/p2p/relays", config.api_url()))
					.headers({
						let mut map = HeaderMap::new();
						map.insert(
							header::AUTHORIZATION,
							HeaderValue::from_str(&format!("Bearer {}", config.p2p_secret))
								.unwrap(),
						);
						map
					})
					.json(&RelayServerEntry {
						id: config.id,
						peer_id: config.keypair.public().to_peer_id().to_base58(),
						addrs: {
							let mut ips: Vec<SocketAddr> =
								vec![SocketAddr::from((public_ipv4, config.port()))];
							if let Some(ip) = public_ipv6 {
								ips.push(SocketAddr::from((ip, config.port())));
							}
							ips
						},
					})
					.send()
					.await;

				let mut is_ok = result.is_ok();
				match result {
					Ok(result) => {
						if result.status() != 200 {
							error!(
								"Failed to register relay server with cloud status {}: {:?}",
								result.status(),
								result.text().await
							);
							is_ok = false;
						} else {
							info!(
								"Successfully registered '{}' as relay server with cloud",
								config.id
							);
						}
					}
					Err(e) => error!("Failed to register relay server with cloud: {e}"),
				}

				if let Some(tx) = first_advertisement_tx.take() {
					tx.send(is_ok).await.ok();
				}

				tokio::time::sleep(std::time::Duration::from_secs(9 * 60)).await;
			}
		}
	});

	if !first_advertisement_rx
		.recv()
		.await
		.expect("Advertisement task died during startup!")
	{
		panic!(
			"Failed to register relay server with cloud. Please check your config and try again."
		); // TODO: Error handling
	}

	// TODO: Setup logging to filesystem with auto-rotation

	let peer_id = config.keypair.public().to_peer_id();

	let mut swarm = libp2p::SwarmBuilder::with_existing_identity(config.keypair.clone())
		.with_tokio()
		.with_quic()
		.with_behaviour(|key| Behaviour {
			relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()), // TODO: Proper config
			autonat: autonat::Behaviour::new(key.public().to_peer_id(), Default::default()), // TODO: Proper config
		})
		.unwrap() // TODO: Error handling
		.build();

	swarm
		.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
			Ipv6Addr::UNSPECIFIED,
			config.port(),
		))))
		.unwrap(); // TODO: Error handling
	swarm
		.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
			Ipv4Addr::UNSPECIFIED,
			config.port(),
		))))
		.unwrap(); // TODO: Error handling

	info!("Started Relay as PeerId '{peer_id}'");

	loop {
		match swarm.next().await.expect("Infinite Stream.") {
			// SwarmEvent::Behaviour(event) => {
			// 	println!("{event:?}")
			// }
			SwarmEvent::NewListenAddr { address, .. } => {
				info!("Listening on {address:?}");
			}
			event => println!("{event:?}"),
		}
	}
}
