//! Unified LibP2P behavior combining all networking protocols

pub use crate::infrastructure::networking::protocols::{pairing::PairingMessage, file_transfer::FileTransferMessage};
use libp2p::{
	kad::{self, store::MemoryStore},
	mdns,
	request_response::{self, ProtocolSupport},
	swarm::NetworkBehaviour,
	StreamProtocol,
};
use serde::{Deserialize, Serialize};

/// Unified behavior that combines all LibP2P protocols
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "UnifiedBehaviourEvent")]
pub struct UnifiedBehaviour {
	/// Kademlia DHT for peer discovery and content routing
	pub kademlia: kad::Behaviour<MemoryStore>,

	/// mDNS for local network discovery
	pub mdns: mdns::tokio::Behaviour,

	/// Request-response for pairing protocol (using CBOR)
	pub pairing: request_response::cbor::Behaviour<PairingMessage, PairingMessage>,

	/// Request-response for device messaging (using CBOR)
	pub messaging: request_response::cbor::Behaviour<DeviceMessage, DeviceMessage>,

	/// Request-response for file transfer (using CBOR)
	pub file_transfer: request_response::cbor::Behaviour<FileTransferMessage, FileTransferMessage>,
}

/// Events from the unified behavior
#[derive(Debug)]
pub enum UnifiedBehaviourEvent {
	Kademlia(kad::Event),
	Mdns(mdns::Event),
	Pairing(request_response::Event<PairingMessage, PairingMessage>),
	Messaging(request_response::Event<DeviceMessage, DeviceMessage>),
	FileTransfer(request_response::Event<FileTransferMessage, FileTransferMessage>),
}

impl From<kad::Event> for UnifiedBehaviourEvent {
	fn from(event: kad::Event) -> Self {
		UnifiedBehaviourEvent::Kademlia(event)
	}
}

impl From<mdns::Event> for UnifiedBehaviourEvent {
	fn from(event: mdns::Event) -> Self {
		UnifiedBehaviourEvent::Mdns(event)
	}
}

impl From<request_response::Event<PairingMessage, PairingMessage>> for UnifiedBehaviourEvent {
	fn from(event: request_response::Event<PairingMessage, PairingMessage>) -> Self {
		UnifiedBehaviourEvent::Pairing(event)
	}
}

impl From<request_response::Event<DeviceMessage, DeviceMessage>> for UnifiedBehaviourEvent {
	fn from(event: request_response::Event<DeviceMessage, DeviceMessage>) -> Self {
		UnifiedBehaviourEvent::Messaging(event)
	}
}

impl From<request_response::Event<FileTransferMessage, FileTransferMessage>> for UnifiedBehaviourEvent {
	fn from(event: request_response::Event<FileTransferMessage, FileTransferMessage>) -> Self {
		UnifiedBehaviourEvent::FileTransfer(event)
	}
}

impl UnifiedBehaviour {
	pub fn new(local_peer_id: libp2p::PeerId) -> Result<Self, Box<dyn std::error::Error>> {
		// Configure Kademlia DHT
		let mut kademlia_config = kad::Config::default();
		kademlia_config.set_query_timeout(std::time::Duration::from_secs(30));
		kademlia_config.set_record_ttl(Some(std::time::Duration::from_secs(3600))); // 1 hour

		let kademlia = kad::Behaviour::with_config(
			local_peer_id,
			MemoryStore::new(local_peer_id),
			kademlia_config,
		);

		// Configure mDNS for local discovery (use default config to match working test)
		let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
		println!("🔧 mDNS: Using default configuration for compatibility with working test");

		// Configure request-response for pairing using CBOR codec with longer timeouts
		let mut pairing_config = request_response::Config::default();
		pairing_config = pairing_config.with_request_timeout(std::time::Duration::from_secs(30)); // 30s request timeout
		let pairing = request_response::cbor::Behaviour::new(
			std::iter::once((
				StreamProtocol::new("/spacedrive/pairing/1.0.0"),
				ProtocolSupport::Full,
			)),
			pairing_config,
		);

		// Configure request-response for device messaging using CBOR codec with longer timeouts  
		let mut messaging_config = request_response::Config::default();
		messaging_config = messaging_config.with_request_timeout(std::time::Duration::from_secs(30)); // 30s request timeout
		let messaging = request_response::cbor::Behaviour::new(
			std::iter::once((
				StreamProtocol::new("/spacedrive/device/1.0.0"),
				ProtocolSupport::Full,
			)),
			messaging_config,
		);

		// Configure request-response for file transfer using CBOR codec with longer timeouts for large files
		let mut file_transfer_config = request_response::Config::default();
		file_transfer_config = file_transfer_config.with_request_timeout(std::time::Duration::from_secs(300)); // 5 min timeout for file operations
		let file_transfer = request_response::cbor::Behaviour::new(
			std::iter::once((
				StreamProtocol::new("/spacedrive/file-transfer/1.0.0"),
				ProtocolSupport::Full,
			)),
			file_transfer_config,
		);

		println!("🔧 Request-Response: Configured 30s request timeout to prevent timeouts");
		println!("🔧 File Transfer: Configured 5min request timeout for large file operations");

		Ok(Self {
			kademlia,
			mdns,
			pairing,
			messaging,
			file_transfer,
		})
	}
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceMessage {
	// Ping message for connection testing
	Ping {
		timestamp: chrono::DateTime<chrono::Utc>,
	},
	// Pong response
	Pong {
		timestamp: chrono::DateTime<chrono::Utc>,
	},
	// Generic protocol message
	Protocol {
		protocol: String,
		data: Vec<u8>,
	},
}
