//! Pairing protocol types and state definitions

use crate::service::network::{
	device::{DeviceInfo, SessionKeys},
	utils::identity::NetworkFingerprint,
};
use chrono::{DateTime, Utc};
use iroh::{NodeAddr, NodeId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Human-readable pairing code using BIP39 mnemonic words
#[derive(Debug, Clone)]
pub struct PairingCode {
	/// 256-bit cryptographic secret
	secret: [u8; 32],

	/// 12 words from BIP39 wordlist for user-friendly sharing
	words: [String; 12],

	/// Session ID derived from secret
	session_id: Uuid,

	/// Expiration timestamp
	expires_at: DateTime<Utc>,

	/// Initiator's NodeId for remote discovery via pkarr (optional - enables relay path)
	node_id: Option<NodeId>,
}

impl PairingCode {
	/// Generate a new pairing code using BIP39 wordlist
	pub fn generate() -> crate::service::network::Result<Self> {
		use rand::RngCore;

		let mut secret = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret);

		// Convert secret to 12 BIP39 words using proper mnemonic encoding
		let words = Self::encode_to_bip39_words(&secret)?;

		// Derive session ID from secret
		let session_id = Self::derive_session_id(&secret);

		Ok(PairingCode {
			secret,
			words,
			session_id,
			expires_at: Utc::now() + chrono::Duration::minutes(5),
			node_id: None,
		})
	}

	/// Add node_id for remote pairing via pkarr discovery
	pub fn with_node_id(mut self, node_id: NodeId) -> Self {
		self.node_id = Some(node_id);
		self
	}

	/// Parse a pairing code from a BIP39 mnemonic string (for local pairing)
	pub fn from_string(code: &str) -> crate::service::network::Result<Self> {
		// Trim the input and normalize whitespace
		let trimmed = code.trim();
		if trimmed.is_empty() {
			return Err(crate::service::network::NetworkingError::Protocol(
				"Pairing code cannot be empty".to_string(),
			));
		}

		let words: Vec<String> = trimmed
			.split_whitespace()
			.map(|s| s.to_lowercase())
			.collect();

		if words.len() != 12 {
			return Err(crate::service::network::NetworkingError::Protocol(format!(
				"Invalid pairing code format - expected 12 words but got {}",
				words.len()
			)));
		}

		// Convert Vec to array
		let words_array: [String; 12] = words.try_into().map_err(|_| {
			crate::service::network::NetworkingError::Protocol(
				"Failed to convert words to array".to_string(),
			)
		})?;

		Self::from_words(&words_array)
	}

	/// Parse a pairing code from QR code JSON (for remote pairing)
	/// Version 2 format: {version, words, node_id} - session_id is derived from words
	pub fn from_qr_json(json: &str) -> crate::service::network::Result<Self> {
		let data: serde_json::Value = serde_json::from_str(json).map_err(|e| {
			crate::service::network::NetworkingError::Protocol(format!(
				"Failed to parse QR code JSON: {}",
				e
			))
		})?;

		// Extract words (BIP39 mnemonic) - required
		let words_str = data.get("words").and_then(|v| v.as_str()).ok_or_else(|| {
			crate::service::network::NetworkingError::Protocol(
				"Missing words in QR code".to_string(),
			)
		})?;

		// Parse words to get the base pairing code (session_id is derived from words)
		let mut code = Self::from_string(words_str)?;

		// Extract node_id (optional - enables remote pairing via pkarr)
		if let Some(node_id_str) = data.get("node_id").and_then(|v| v.as_str()) {
			let node_id = node_id_str.parse::<NodeId>().map_err(|e| {
				crate::service::network::NetworkingError::Protocol(format!(
					"Invalid node_id in QR code: {}",
					e
				))
			})?;
			code.node_id = Some(node_id);
		}

		Ok(code)
	}

	/// Create pairing code from BIP39 words
	pub fn from_words(words: &[String; 12]) -> crate::service::network::Result<Self> {
		// Decode BIP39 words back to secret
		let secret = Self::decode_from_bip39_words(words)?;

		// Derive session ID from the secret
		// This ensures both the initiator and joiner get the same session_id
		let session_id = Self::derive_session_id(&secret);

		Ok(PairingCode {
			secret,
			words: words.clone(),
			session_id,
			expires_at: Utc::now() + chrono::Duration::minutes(5),
			node_id: None,
		})
	}

	/// Get the session ID from this pairing code
	pub fn session_id(&self) -> Uuid {
		self.session_id
	}

	/// Get the cryptographic secret
	pub fn secret(&self) -> &[u8; 32] {
		&self.secret
	}

	/// Get the initiator's NodeId for pkarr discovery
	pub fn node_id(&self) -> Option<NodeId> {
		self.node_id
	}

	/// Convert to display string (for local pairing - BIP39 words only)
	pub fn to_string(&self) -> String {
		self.words.join(" ")
	}

	/// Convert to QR code JSON (for remote pairing)
	/// Version 2: {version, words, node_id} - session_id derived from words, relay discovered via pkarr
	pub fn to_qr_json(&self) -> String {
		serde_json::json!({
			"version": 2,
			"words": self.to_string(),
			"node_id": self.node_id.map(|id| id.to_string()),
		})
		.to_string()
	}

	/// Check if the code has expired
	pub fn is_expired(&self) -> bool {
		Utc::now() > self.expires_at
	}

	/// Encode bytes to BIP39 words using proper mnemonic generation
	fn encode_to_bip39_words(secret: &[u8; 32]) -> crate::service::network::Result<[String; 12]> {
		use bip39::{Language, Mnemonic};

		// For 12 words, we need 128 bits of entropy (standard BIP39)
		// Use the first 16 bytes from our 32-byte secret
		let entropy = &secret[..16];

		// Generate mnemonic from entropy
		let mnemonic = Mnemonic::from_entropy(entropy).map_err(|e| {
			crate::service::network::NetworkingError::Protocol(format!(
				"BIP39 generation failed: {}",
				e
			))
		})?;

		// Get the word list (should be exactly 12 words for 128 bits of entropy)
		let word_list: Vec<&str> = mnemonic.words().collect();

		if word_list.len() != 12 {
			return Err(crate::service::network::NetworkingError::Protocol(format!(
				"Expected 12 words, got {}",
				word_list.len()
			)));
		}

		Ok([
			word_list[0].to_string(),
			word_list[1].to_string(),
			word_list[2].to_string(),
			word_list[3].to_string(),
			word_list[4].to_string(),
			word_list[5].to_string(),
			word_list[6].to_string(),
			word_list[7].to_string(),
			word_list[8].to_string(),
			word_list[9].to_string(),
			word_list[10].to_string(),
			word_list[11].to_string(),
		])
	}

	/// Decode BIP39 words back to secret
	fn decode_from_bip39_words(words: &[String; 12]) -> crate::service::network::Result<[u8; 32]> {
		use bip39::{Language, Mnemonic};

		// Join words with spaces to create mnemonic string
		let mnemonic_str = words.join(" ");

		// Parse the mnemonic
		let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_str).map_err(|e| {
			crate::service::network::NetworkingError::Protocol(format!(
				"Invalid BIP39 mnemonic: {}",
				e
			))
		})?;

		// Extract the entropy (should be 16 bytes for 12 words)
		let entropy = mnemonic.to_entropy();

		if entropy.len() != 16 {
			return Err(crate::service::network::NetworkingError::Protocol(format!(
				"Expected 16 bytes of entropy, got {}",
				entropy.len()
			)));
		}

		// Reconstruct the full 32-byte secret
		// Use the 16 bytes of entropy and derive the remaining 16 bytes deterministically
		let mut full_secret = [0u8; 32];
		full_secret[..16].copy_from_slice(&entropy);

		// Derive the remaining 16 bytes using BLAKE3 for deterministic padding
		let mut hasher = blake3::Hasher::new();
		hasher.update(b"spacedrive-pairing-entropy-extension-v1");
		hasher.update(&entropy);
		let derived_bytes = hasher.finalize();
		full_secret[16..].copy_from_slice(&derived_bytes.as_bytes()[..16]);

		Ok(full_secret)
	}

	/// Derive session ID from secret
	fn derive_session_id(secret: &[u8; 32]) -> Uuid {
		// For pairing codes, derive session ID from the entropy that survives BIP39 round-trip
		// This ensures Initiator (who generates) and Joiner (who parses) get the same session ID
		// This is critical for DHT-based pairing where session IDs must match
		let hash = blake3::hash(&secret[..16]); // Use only the first 16 bytes (BIP39 entropy)
		let bytes = hash.as_bytes();

		let uuid_bytes = [
			bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
			bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
		];

		Uuid::from_bytes(uuid_bytes)
	}
}

impl std::fmt::Display for PairingCode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.to_string())
	}
}

/// State of a pairing session
#[derive(Debug, Clone)]
pub struct PairingSession {
	pub id: Uuid,
	pub state: PairingState,
	pub remote_device_id: Option<Uuid>,
	pub remote_device_info: Option<DeviceInfo>,
	pub remote_public_key: Option<Vec<u8>>,
	pub shared_secret: Option<Vec<u8>>,
	pub created_at: DateTime<Utc>,
}

impl std::fmt::Display for PairingSession {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
            f,
            "PairingSession {{ id: {}, state: {}, remote_device_id: {:?}, shared_secret: {}, created_at: {} }}",
            self.id,
            self.state,
            self.remote_device_id,
            self.shared_secret.as_ref().map(|_| "[PRESENT]").unwrap_or("None"),
            self.created_at
        )
	}
}

/// States of the pairing process
#[derive(Debug, Clone)]
pub enum PairingState {
	Idle,
	GeneratingCode,
	Broadcasting,
	Scanning,
	WaitingForConnection,
	Connecting,
	Authenticating,
	ExchangingKeys,
	AwaitingConfirmation,
	EstablishingSession,
	ChallengeReceived {
		challenge: Vec<u8>,
	},
	ResponsePending {
		challenge: Vec<u8>,
		response_data: Vec<u8>,
		remote_node_id: Option<NodeId>,
	},
	ResponseSent,
	Completed,
	Failed {
		reason: String,
	},
}

impl std::fmt::Display for PairingState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PairingState::ResponsePending {
				challenge,
				response_data,
				..
			} => {
				write!(
					f,
					"ResponsePending {{ challenge: [{}], response_data: [{}], .. }}",
					if challenge.len() > 8 {
						format!("{} items (truncated)", challenge.len())
					} else {
						challenge
							.iter()
							.map(|b| b.to_string())
							.collect::<Vec<_>>()
							.join(", ")
					},
					if response_data.len() > 8 {
						format!("{} items (truncated)", response_data.len())
					} else {
						response_data
							.iter()
							.map(|b| b.to_string())
							.collect::<Vec<_>>()
							.join(", ")
					}
				)
			}
			PairingState::ChallengeReceived { challenge } => {
				write!(
					f,
					"ChallengeReceived {{ challenge: [{}] }}",
					if challenge.len() > 8 {
						format!("{} items (truncated)", challenge.len())
					} else {
						challenge
							.iter()
							.map(|b| b.to_string())
							.collect::<Vec<_>>()
							.join(", ")
					}
				)
			}
			_ => write!(f, "{:?}", self),
		}
	}
}

/// Role in the pairing process
#[derive(Debug, Clone, PartialEq)]
pub enum PairingRole {
	Initiator,
	Joiner,
}

/// Discovery advertisement for pairing session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingAdvertisement {
	/// The node ID of the initiator (as string for serialization)
	pub node_id: String,
	/// The node address components for reconstruction
	pub node_addr_info: NodeAddrInfo,
	/// Device information of the initiator
	pub device_info: DeviceInfo,
	/// When this advertisement expires
	pub expires_at: DateTime<Utc>,
	/// When this advertisement was created
	pub created_at: DateTime<Utc>,
}

/// Serializable representation of NodeAddr
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddrInfo {
	/// Node ID as string
	pub node_id: String,
	/// Direct socket addresses
	pub direct_addresses: Vec<String>,
	/// Relay URL if available
	pub relay_url: Option<String>,
}

impl PairingAdvertisement {
	/// Convert node ID string back to NodeId
	pub fn node_id(&self) -> crate::service::network::Result<NodeId> {
		self.node_id.parse().map_err(|e| {
			crate::service::network::NetworkingError::Protocol(format!("Invalid node ID: {}", e))
		})
	}

	/// Convert node address info back to NodeAddr
	pub fn node_addr(&self) -> crate::service::network::Result<NodeAddr> {
		// Parse node ID
		let node_id = self.node_addr_info.node_id.parse::<NodeId>().map_err(|e| {
			crate::service::network::NetworkingError::Protocol(format!(
				"Invalid node ID in advertisement: {}",
				e
			))
		})?;

		// Start with base NodeAddr
		let mut node_addr = NodeAddr::new(node_id);

		// Add direct addresses
		let mut direct_addrs = Vec::new();
		for addr_str in &self.node_addr_info.direct_addresses {
			if let Ok(addr) = addr_str.parse() {
				direct_addrs.push(addr);
			}
		}
		if !direct_addrs.is_empty() {
			node_addr = node_addr.with_direct_addresses(direct_addrs);
		}

		// Add relay URL if present
		if let Some(relay_url) = &self.node_addr_info.relay_url {
			if let Ok(url) = relay_url.parse() {
				node_addr = node_addr.with_relay_url(url);
			}
		}

		Ok(node_addr)
	}
}
