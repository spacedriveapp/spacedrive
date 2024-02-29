use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use libp2p::{futures::StreamExt, identity::Keypair, relay, swarm::SwarmEvent};
use tracing::info;

use crate::utils::socketaddr_to_quic_multiaddr;

mod config;
mod utils;

// TODO: Authentication with the Spacedrive Cloud
// TODO: Rate-limit data usage by Spacedrive account.

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt::init();

	// let config_path = std::env::var("CONFIG_PATH").unwrap_or("./config.toml".to_string());
	// println!("{:?}", config_path);
	// TODO: Get port from config
	let port = 7373; // TODO: Should we use HTTPS port to avoid strict internet filters???

	// TODO: Setup logging to filesystem with auto-rotation

	// TODO: pull this from the config so it's consistent
	let local_key = Keypair::generate_ed25519();
	let peer_id = local_key.public().to_peer_id();

	let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
		.with_tokio()
		.with_quic()
		.with_behaviour(|key| relay::Behaviour::new(key.public().to_peer_id(), Default::default()))
		.unwrap() // TODO: Error handling
		.build();

	swarm
		.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
			Ipv6Addr::UNSPECIFIED,
			port,
		))))
		.unwrap(); // TODO: Error handling
	swarm
		.listen_on(socketaddr_to_quic_multiaddr(&SocketAddr::from((
			Ipv4Addr::UNSPECIFIED,
			port,
		))))
		.unwrap(); // TODO: Error handling

	info!("Started Relay as PeerId '{peer_id}'");

	loop {
		match swarm.next().await.expect("Infinite Stream.") {
			SwarmEvent::Behaviour(event) => {
				println!("{event:?}")
			}
			SwarmEvent::NewListenAddr { address, .. } => {
				info!("Listening on {address:?}");
			}
			_ => {}
		}
	}
}