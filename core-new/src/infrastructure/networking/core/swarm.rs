//! LibP2P swarm creation and configuration

use super::{behavior::UnifiedBehaviour, NetworkingError, Result};
use crate::infrastructure::networking::utils::NetworkIdentity;
use libp2p::{noise, swarm::Swarm, tcp, yamux, Multiaddr, PeerId, Transport};

/// Create a new LibP2P swarm with unified behavior
pub async fn create_swarm(identity: NetworkIdentity) -> Result<Swarm<UnifiedBehaviour>> {
	let local_peer_id = identity.peer_id();

	// Create transport stack
	let transport = create_transport(&identity).await?;

	// Create unified behavior
	let behaviour = UnifiedBehaviour::new(local_peer_id)
		.map_err(|e| NetworkingError::Protocol(e.to_string()))?;

	// Build swarm with default config
	let config = libp2p::swarm::Config::with_tokio_executor();
	let mut swarm = Swarm::new(transport, behaviour, local_peer_id, config);

	// Configure external addresses for local testing
	configure_external_addresses(&mut swarm);

	Ok(swarm)
}

/// Create the transport stack with TCP-only, Noise encryption, and Yamux multiplexing
/// Simplified to match working mDNS test configuration (no QUIC complexity)
async fn create_transport(
	identity: &NetworkIdentity,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>> {
	let keypair = identity.keypair().clone();

	// Create Noise authentication
	let noise_config = noise::Config::new(&keypair)
		.map_err(|e| NetworkingError::Protocol(format!("Noise config error: {}", e)))?;

	// Create TCP-only transport with keep-alive configuration
	let mut tcp_config = tcp::Config::default();
	tcp_config = tcp_config.nodelay(true);
	
	// Configure Yamux with default settings (libp2p version doesn't expose many config options)
	let yamux_config = yamux::Config::default();
	
	let transport = tcp::tokio::Transport::new(tcp_config)
		.upgrade(libp2p::core::upgrade::Version::V1)
		.authenticate(noise_config)
		.multiplex(yamux_config)
		.boxed();

	println!("🔧 Transport: Using TCP-only configuration (no QUIC) for connection stability");

	Ok(transport)
}

/// Configure external addresses for the swarm
fn configure_external_addresses(swarm: &mut Swarm<UnifiedBehaviour>) {
	// Add local addresses that might be accessible
	let local_addresses = ["/ip4/127.0.0.1/tcp/0", "/ip4/0.0.0.0/tcp/0"];

	for addr_str in &local_addresses {
		if let Ok(addr) = addr_str.parse::<Multiaddr>() {
			swarm.add_external_address(addr);
		}
	}
}
