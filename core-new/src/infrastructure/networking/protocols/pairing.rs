//! Pairing protocol handler

use super::{ProtocolEvent, ProtocolHandler};
use crate::infrastructure::networking::{
	device::{DeviceInfo, DeviceRegistry, SessionKeys},
	utils::NetworkIdentity,
	NetworkingError, Result,
};
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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
	expires_at: chrono::DateTime<chrono::Utc>,
}

impl PairingCode {
	/// Generate a new pairing code using BIP39 wordlist
	pub fn generate() -> Result<Self> {
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
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
		})
	}

	/// Generate a pairing code from a session ID (for compatibility)
	pub fn from_session_id(session_id: Uuid) -> Self {
		// Use session ID bytes as basis for secret
		let session_bytes = session_id.as_bytes();
		let mut secret = [0u8; 32];

		// Derive full secret from session ID using BLAKE3
		let hash = blake3::hash(session_bytes);
		secret.copy_from_slice(hash.as_bytes());

		// Generate BIP39 words from secret
		let words = Self::encode_to_bip39_words(&secret).unwrap_or_else(|_| {
			// Fallback to empty words if BIP39 fails
			[const { String::new() }; 12]
		});

		Self {
			secret,
			words,
			session_id,
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
		}
	}

	/// Parse a pairing code from a BIP39 mnemonic string
	pub fn from_string(code: &str) -> Result<Self> {
		let words: Vec<String> = code.split_whitespace().map(|s| s.to_lowercase()).collect();

		if words.len() != 12 {
			return Err(NetworkingError::Protocol(
				"Invalid pairing code format - must be 12 BIP39 words".to_string(),
			));
		}

		// Convert Vec to array
		let words_array: [String; 12] = words.try_into().map_err(|_| {
			NetworkingError::Protocol("Failed to convert words to array".to_string())
		})?;

		Self::from_words(&words_array)
	}

	/// Create pairing code from BIP39 words
	pub fn from_words(words: &[String; 12]) -> Result<Self> {
		// Decode BIP39 words back to secret
		let secret = Self::decode_from_bip39_words(words)?;

		// Derive session ID from secret
		let session_id = Self::derive_session_id(&secret);

		Ok(PairingCode {
			secret,
			words: words.clone(),
			session_id,
			expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
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

	/// Convert to display string
	pub fn to_string(&self) -> String {
		self.words.join(" ")
	}

	/// Check if the code has expired
	pub fn is_expired(&self) -> bool {
		chrono::Utc::now() > self.expires_at
	}

	/// Encode bytes to BIP39 words using proper mnemonic generation
	fn encode_to_bip39_words(secret: &[u8; 32]) -> Result<[String; 12]> {
		use bip39::{Language, Mnemonic};

		// For 12 words, we need 128 bits of entropy (standard BIP39)
		// Use the first 16 bytes from our 32-byte secret
		let entropy = &secret[..16];

		// Generate mnemonic from entropy
		let mnemonic = Mnemonic::from_entropy(entropy)
			.map_err(|e| NetworkingError::Protocol(format!("BIP39 generation failed: {}", e)))?;

		// Get the word list (should be exactly 12 words for 128 bits of entropy)
		let word_list: Vec<&str> = mnemonic.words().collect();

		if word_list.len() != 12 {
			return Err(NetworkingError::Protocol(format!(
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
	fn decode_from_bip39_words(words: &[String; 12]) -> Result<[u8; 32]> {
		use bip39::{Language, Mnemonic};

		// Join words with spaces to create mnemonic string
		let mnemonic_str = words.join(" ");

		// Parse the mnemonic
		let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_str)
			.map_err(|e| NetworkingError::Protocol(format!("Invalid BIP39 mnemonic: {}", e)))?;

		// Extract the entropy (should be 16 bytes for 12 words)
		let entropy = mnemonic.to_entropy();

		if entropy.len() != 16 {
			return Err(NetworkingError::Protocol(format!(
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
		// Use BLAKE3 to derive UUID from secret
		let hash = blake3::hash(secret);
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

/// Pairing protocol handler
pub struct PairingProtocolHandler {
	/// Network identity for signing
	identity: NetworkIdentity,

	/// Device registry for state management
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Active pairing sessions
	active_sessions: Arc<RwLock<HashMap<Uuid, PairingSession>>>,
}

/// State of a pairing session
#[derive(Debug, Clone)]
pub struct PairingSession {
	pub id: Uuid,
	pub state: PairingState,
	pub remote_device_id: Option<Uuid>,
	pub shared_secret: Option<Vec<u8>>,
	pub created_at: chrono::DateTime<chrono::Utc>,
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
	ChallengeReceived { challenge: Vec<u8> },
	ResponseSent,
	Completed,
	Failed { reason: String },
}

/// DHT advertisement for pairing session discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingAdvertisement {
	/// The peer ID of the initiator (as string for serialization)
	pub peer_id: String,
	/// The network addresses where the initiator can be reached (as strings for serialization)
	pub addresses: Vec<String>,
	/// Device information of the initiator
	pub device_info: DeviceInfo,
	/// When this advertisement expires
	pub expires_at: chrono::DateTime<chrono::Utc>,
	/// When this advertisement was created
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PairingAdvertisement {
	/// Convert peer ID string back to PeerId
	pub fn peer_id(&self) -> Result<PeerId> {
		self.peer_id.parse()
			.map_err(|e| NetworkingError::Protocol(format!("Invalid peer ID: {}", e)))
	}

	/// Convert address strings back to Multiaddr
	pub fn addresses(&self) -> Result<Vec<Multiaddr>> {
		self.addresses.iter()
			.map(|addr| addr.parse()
				.map_err(|e| NetworkingError::Protocol(format!("Invalid address: {}", e))))
			.collect()
	}
}

/// Pairing messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PairingMessage {
	/// Initial pairing request
	Request {
		session_id: Uuid,
		device_info: DeviceInfo,
		public_key: Vec<u8>,
	},
	/// Challenge for authentication
	Challenge {
		session_id: Uuid,
		challenge: Vec<u8>,
	},
	/// Response to challenge
	Response {
		session_id: Uuid,
		response: Vec<u8>,
		device_info: DeviceInfo,
	},
	/// Pairing completion
	Complete {
		session_id: Uuid,
		success: bool,
		reason: Option<String>,
	},
}

impl PairingProtocolHandler {
	/// Create a new pairing protocol handler
	pub fn new(identity: NetworkIdentity, device_registry: Arc<RwLock<DeviceRegistry>>) -> Self {
		Self {
			identity,
			device_registry,
			active_sessions: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Start a new pairing session as initiator
	/// Returns the session ID which should be advertised via DHT by the caller
	pub async fn start_pairing_session(&self) -> Result<Uuid> {
		let session_id = Uuid::new_v4();
		let session = PairingSession {
			id: session_id,
			state: PairingState::WaitingForConnection,
			remote_device_id: None,
			shared_secret: None,
			created_at: chrono::Utc::now(),
		};

		self.active_sessions
			.write()
			.await
			.insert(session_id, session);

		println!("Started pairing session: {}", session_id);
		Ok(session_id)
	}

	/// Get device info for advertising in DHT records
	pub fn get_device_info(&self) -> DeviceInfo {
		DeviceInfo {
			device_id: self.identity.device_id(),
			device_name: "Spacedrive Device".to_string(), // TODO: Get from device manager
			device_type: crate::infrastructure::networking::device::DeviceType::Desktop, // TODO: Get from device manager
			os_version: std::env::consts::OS.to_string(),
			app_version: "0.1.0".to_string(), // TODO: Get from application version
			network_fingerprint: self.identity.network_fingerprint(),
			last_seen: chrono::Utc::now(),
		}
	}

	/// Cancel a pairing session
	pub async fn cancel_session(&self, session_id: Uuid) -> Result<()> {
		self.active_sessions.write().await.remove(&session_id);
		Ok(())
	}

	/// Get active pairing sessions
	pub async fn get_active_sessions(&self) -> Vec<PairingSession> {
		self.active_sessions
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}

	async fn handle_pairing_request(
		&self,
		from_device: Uuid,
		session_id: Uuid,
		device_info: DeviceInfo,
		public_key: Vec<u8>,
	) -> Result<Vec<u8>> {
		// Generate challenge
		let challenge = self.generate_challenge()?;

		// Store session
		let session = PairingSession {
			id: session_id,
			state: PairingState::ChallengeReceived {
				challenge: challenge.clone(),
			},
			remote_device_id: Some(from_device),
			shared_secret: None,
			created_at: chrono::Utc::now(),
		};

		self.active_sessions
			.write()
			.await
			.insert(session_id, session);

		// Send challenge response
		let response = PairingMessage::Challenge {
			session_id,
			challenge,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_pairing_challenge(
		&self,
		session_id: Uuid,
		challenge: Vec<u8>,
	) -> Result<Vec<u8>> {
		// Sign the challenge
		let signature = self.identity.sign(&challenge)?;

		// Get local device info
		let device_info = self.device_registry.read().await.get_local_device_info()?;

		// Update session state
		if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
			session.state = PairingState::ResponseSent;
		}

		// Send response
		let response = PairingMessage::Response {
			session_id,
			response: signature,
			device_info,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_pairing_response(
		&self,
		from_device: Uuid,
		session_id: Uuid,
		response: Vec<u8>,
		device_info: DeviceInfo,
	) -> Result<Vec<u8>> {
		// Verify the response signature
		let session = self
			.active_sessions
			.read()
			.await
			.get(&session_id)
			.cloned()
			.ok_or_else(|| NetworkingError::Protocol("Session not found".to_string()))?;

		let challenge = match &session.state {
			PairingState::ChallengeReceived { challenge } => challenge.clone(),
			_ => {
				return Err(NetworkingError::Protocol(
					"Invalid session state".to_string(),
				))
			}
		};

		// In a real implementation, we'd verify the signature using the remote device's public key
		// For now, we'll assume the signature is valid

		// Generate session keys
		let shared_secret = self.generate_shared_secret()?;
		let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

		// Complete pairing in device registry
		self.device_registry.write().await.complete_pairing(
			from_device,
			device_info,
			session_keys,
		)?;

		// Update session
		if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
			session.state = PairingState::Completed;
			session.shared_secret = Some(shared_secret);
		}

		// Send completion message
		let response = PairingMessage::Complete {
			session_id,
			success: true,
			reason: None,
		};

		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	fn generate_challenge(&self) -> Result<Vec<u8>> {
		use rand::RngCore;
		let mut challenge = vec![0u8; 32];
		rand::thread_rng().fill_bytes(&mut challenge);
		Ok(challenge)
	}

	fn generate_shared_secret(&self) -> Result<Vec<u8>> {
		use rand::RngCore;
		let mut secret = vec![0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret);
		Ok(secret)
	}
}

#[async_trait]
impl ProtocolHandler for PairingProtocolHandler {
	fn protocol_name(&self) -> &str {
		"pairing"
	}

	async fn handle_request(&self, from_device: Uuid, request_data: Vec<u8>) -> Result<Vec<u8>> {
		let message: PairingMessage =
			serde_json::from_slice(&request_data).map_err(|e| NetworkingError::Serialization(e))?;

		match message {
			PairingMessage::Request {
				session_id,
				device_info,
				public_key,
			} => {
				self.handle_pairing_request(from_device, session_id, device_info, public_key)
					.await
			}
			PairingMessage::Challenge {
				session_id,
				challenge,
			} => self.handle_pairing_challenge(session_id, challenge).await,
			PairingMessage::Response {
				session_id,
				response,
				device_info,
			} => {
				self.handle_pairing_response(from_device, session_id, response, device_info)
					.await
			}
			PairingMessage::Complete {
				session_id,
				success,
				reason,
			} => {
				// Handle completion acknowledgment
				if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
					if success {
						session.state = PairingState::Completed;
					} else {
						session.state = PairingState::Failed {
							reason: reason.unwrap_or_else(|| "Unknown error".to_string()),
						};
					}
				}

				// Return empty response for completion
				Ok(Vec::new())
			}
		}
	}

	async fn handle_response(&self, _from_device: Uuid, _response_data: Vec<u8>) -> Result<()> {
		// Pairing protocol doesn't use separate responses
		Ok(())
	}

	async fn handle_event(&self, event: ProtocolEvent) -> Result<()> {
		match event {
			ProtocolEvent::DeviceDisconnected { device_id } => {
				// Clean up any active sessions for this device
				let mut sessions = self.active_sessions.write().await;
				sessions.retain(|_, session| session.remote_device_id != Some(device_id));
			}
			_ => {}
		}

		Ok(())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}
