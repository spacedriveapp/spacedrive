//! LibP2P swarm creation and configuration

use super::{behavior::UnifiedBehaviour, NetworkingError, Result};
use crate::infrastructure::networking::utils::NetworkIdentity;
use futures::future::Either;
use libp2p::{noise, quic, swarm::Swarm, tcp, yamux, Multiaddr, PeerId, Transport};

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

/// Create the transport stack with TCP + QUIC, Noise encryption, and Yamux multiplexing
async fn create_transport(
	identity: &NetworkIdentity,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>> {
	let keypair = identity.keypair().clone();

	// Create Noise authentication
	let noise_config = noise::Config::new(&keypair)
		.map_err(|e| NetworkingError::Protocol(format!("Noise config error: {}", e)))?;

	// Create TCP transport
	let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
		.upgrade(libp2p::core::upgrade::Version::V1)
		.authenticate(noise_config.clone())
		.multiplex(yamux::Config::default())
		.boxed();

	// Create QUIC transport
	let quic_transport = quic::tokio::Transport::new(quic::Config::new(&keypair)).boxed();

	// Combine transports
	let transport = tcp_transport
		.or_transport(quic_transport)
		.map(|either_output, _| match either_output {
			Either::Left((peer_id, muxer)) => {
				(peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
			}
			Either::Right((peer_id, muxer)) => {
				(peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
			}
		})
		.boxed();

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
