mod discovery;
mod error;
mod identity;
mod network_manager;
mod network_manager_state;
mod p2p_application;
mod peer;
mod peer_id;
mod quic;

use std::{collections::HashMap, net::Ipv4Addr, sync::Arc};

use dashmap::DashSet;
pub(crate) use discovery::*;
pub use error::*;
pub use identity::*;
pub use network_manager::*;
pub use network_manager_state::*;
pub use p2p_application::*;
pub use peer::*;
pub use peer_id::*;
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, VarInt};

/// NetworkManagerEvent is an event that is sent to the application which is embedding 'sd-p2p'. It allows the application to react to events that occur in the networking layer.
#[derive(Debug)]
pub enum NetworkManagerEvent {
	/// PeerDiscovered is sent when a new peer is discovered which is available be be paired with. It is recommended when this event comes in that you establish a connection with the peer if it is known.
	PeerDiscovered { peer: PeerCandidate },
	/// ConnectionEstablished is sent when a connection is established with a peer.
	ConnectionEstablished { peer: Peer },
	/// AcceptStream is sent when a networking stream is accepted by the server. The stream can be handled by the user or closed.
	AcceptStream {
		peer: Peer,
		stream: (SendStream, RecvStream),
	},
	/// TODO
	PeerRequest { peer: Peer, data: Vec<u8> },
	/// ConnectionClosed is sent when a connection is closed with a peer.
	ConnectionClosed { peer: Peer },
}

/// PeerCandidate represents a peer that has been discovered but not paired with.
#[derive(Debug, Clone)]
pub struct PeerCandidate {
	pub id: PeerId,
	pub metadata: PeerMetadata,
	pub addresses: Vec<Ipv4Addr>,
	pub port: u16,
}

/// PeerMetadata represents public metadata about a peer. This is found through the discovery process.
#[derive(Debug, Clone)]
pub struct PeerMetadata {
	pub name: String,
	pub version: Option<String>,
}

impl PeerMetadata {
	pub fn from_hashmap(peer_id: &PeerId, hashmap: &HashMap<String, String>) -> Self {
		Self {
			name: hashmap
				.get("name")
				.map(|v| v.to_string())
				.unwrap_or(peer_id.to_string()),
			version: hashmap.get("version").map(|v| v.to_string()),
		}
	}

	pub fn to_hashmap(self) -> HashMap<String, String> {
		let mut hashmap = HashMap::new();
		hashmap.insert("name".to_string(), self.name);
		if let Some(version) = self.version {
			hashmap.insert("version".to_string(), version);
		}
		hashmap
	}
}

// TODO: Move into another file
pub struct GlobalDiscovery {
	pub urls: DashSet<String>,
}

impl GlobalDiscovery {
	// TODO: Load from env or get default by reach out to backend.
	pub(crate) fn load_from_env() -> Self {
		// TODO: Load from environment variables

		let urls = DashSet::new();
		urls.insert("127.0.0.1:443".into());
		Self { urls }
	}

	// TODO: Query spacedrive backend for which peers are online and their certificates

	pub(crate) async fn do_client_announcement(&self, endpoint: Endpoint) -> Result<(), ()> {
		// TODO: Connect to random URL, Handle if server is offline, Handle domain name resolution
		let url = self.urls.iter().next().unwrap().clone();
		println!("{}", url);

		let mut client_crypto = rustls::ClientConfig::builder()
			.with_safe_default_cipher_suites()
			.with_safe_default_kx_groups()
			.with_protocol_versions(&[&rustls::version::TLS13])
			.unwrap()
			.with_custom_certificate_verifier(crate::quic::ServerCertificateVerifier::new())
			.with_no_client_auth(); // TODO: Make server certificate verification secure

		// client_crypto.alpn_protocols = crate::quic::ALPN_QUIC_HTTP
		// 	.iter()
		// 	.map(|&x| x.into())
		// 	.collect();

		let new_conn = endpoint
			.connect_with(
				ClientConfig::new(Arc::new(client_crypto.clone())),
				url.parse().unwrap(),
				"todo",
			)
			.map_err(|err| {
				println!("{}", err);
				()
			})?;

		let quinn::NewConnection {
			connection: conn, ..
		} = new_conn.await.map_err(|err| {
			println!("{}", err);
			()
		})?;

		let (mut tx, rx) = conn.open_bi().await.unwrap();

		tx.write(b"Hello").await.unwrap();

		tx.finish().await.unwrap();

		// TODO: Make int and reason constants
		conn.close(VarInt::from_u32(5), b"DONE");

		Ok(())
	}
}
