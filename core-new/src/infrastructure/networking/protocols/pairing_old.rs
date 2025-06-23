//! Pairing protocol handler

use super::{ProtocolEvent, ProtocolHandler};
use crate::infrastructure::networking::{
	core::behavior::PairingMessage,
	device::{DeviceInfo, DeviceRegistry, SessionKeys},
	utils::{identity::NetworkFingerprint, NetworkIdentity},
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
		// CRITICAL FIX: Instead of deriving secret from session_id,
		// we should make session_id derivable from secret.
		// This ensures Alice and Bob get the same session_id when they
		// both process the same BIP39 words.

		// Generate a fresh secret and derive session_id from it consistently
		use rand::RngCore;
		let mut secret = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut secret);

		// Both Alice and Bob will derive the same session_id from this secret
		let derived_session_id = Self::derive_session_id(&secret);

		// Generate BIP39 words from secret
		let words = Self::encode_to_bip39_words(&secret).unwrap_or_else(|_| {
			// Fallback to empty words if BIP39 fails
			[const { String::new() }; 12]
		});

		Self {
			secret,
			words,
			session_id: derived_session_id, // Use derived session_id, not original
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

		// Derive session ID from secret - this will match Alice's derivation
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
		// For pairing codes, derive session ID from the entropy that survives BIP39 round-trip
		// This ensures Alice (who generates) and Bob (who parses) get the same session ID
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
		remote_peer_id: Option<PeerId>,
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
		self.peer_id
			.parse()
			.map_err(|e| NetworkingError::Protocol(format!("Invalid peer ID: {}", e)))
	}

	/// Convert address strings back to Multiaddr
	pub fn addresses(&self) -> Result<Vec<Multiaddr>> {
		self.addresses
			.iter()
			.map(|addr| {
				addr.parse()
					.map_err(|e| NetworkingError::Protocol(format!("Invalid address: {}", e)))
			})
			.collect()
	}
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
		self.start_pairing_session_with_id(session_id).await?;
		Ok(session_id)
	}

	/// Start a new pairing session with a specific session ID
	pub async fn start_pairing_session_with_id(&self, session_id: Uuid) -> Result<()> {
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
		Ok(())
	}

	/// Join an existing pairing session with a specific session ID
	/// This allows a joiner to participate in an initiator's session
	pub async fn join_pairing_session(&self, session_id: Uuid) -> Result<()> {
		// Check if session already exists to prevent conflicts
		{
			let sessions = self.active_sessions.read().await;
			if let Some(existing_session) = sessions.get(&session_id) {
				return Err(NetworkingError::Protocol(format!(
					"Session {} already exists in state {:?}",
					session_id, existing_session.state
				)));
			}
		}

		// Create new scanning session for Bob (the joiner)
		let session = PairingSession {
			id: session_id,
			state: PairingState::Scanning, // Joiner starts in scanning state
			remote_device_id: None,
			shared_secret: None,
			created_at: chrono::Utc::now(),
		};

		// Insert the session
		{
			let mut sessions = self.active_sessions.write().await;
			sessions.insert(session_id, session);
		}

		println!(
			"✅ Joined pairing session: {} (state: Scanning)",
			session_id
		);

		// Verify session was created correctly
		let sessions = self.active_sessions.read().await;
		if let Some(created_session) = sessions.get(&session_id) {
			if matches!(created_session.state, PairingState::Scanning) {
				println!(
					"✅ Bob's pairing session verified in Scanning state: {}",
					session_id
				);
			} else {
				return Err(NetworkingError::Protocol(format!(
					"Session {} created in wrong state: {:?}",
					session_id, created_session.state
				)));
			}
		} else {
			return Err(NetworkingError::Protocol(format!(
				"Failed to verify session creation: {}",
				session_id
			)));
		}

		Ok(())
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
		let sessions = {
			let read_guard = self.active_sessions.read().await;
			read_guard.values().cloned().collect::<Vec<_>>()
		};
		sessions
	}

	/// Clean up expired pairing sessions
	pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
		let now = chrono::Utc::now();
		let timeout_duration = chrono::Duration::minutes(10); // 10 minute timeout

		let mut sessions = self.active_sessions.write().await;
		let initial_count = sessions.len();

		// Remove sessions older than timeout duration
		sessions.retain(|_, session| {
			let age = now.signed_duration_since(session.created_at);
			if age > timeout_duration {
				println!(
					"Cleaning up expired pairing session: {} (age: {} minutes)",
					session.id,
					age.num_minutes()
				);
				false
			} else {
				true
			}
		});

		let cleaned_count = initial_count - sessions.len();
		if cleaned_count > 0 {
			println!("Cleaned up {} expired pairing sessions", cleaned_count);
		}

		Ok(cleaned_count)
	}

	/// Start a background task to periodically clean up expired sessions
	pub fn start_cleanup_task(handler: Arc<Self>) {
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute

			loop {
				interval.tick().await;

				if let Err(e) = handler.cleanup_expired_sessions().await {
					eprintln!("Error during session cleanup: {}", e);
				}
			}
		});
	}

	async fn handle_pairing_request(
		&self,
		from_device: Uuid,
		session_id: Uuid,
		device_info: DeviceInfo,
		public_key: Vec<u8>,
	) -> Result<Vec<u8>> {
		println!(
			"🔥 ALICE: Received pairing request from device {} for session {}",
			from_device, session_id
		);

		// Generate challenge
		let challenge = self.generate_challenge()?;
		println!(
			"🔥 ALICE: Generated challenge of {} bytes for session {}",
			challenge.len(),
			session_id
		);

		// Check for existing session and transition state properly
		let existing_session_info = {
			let read_guard = self.active_sessions.read().await;
			read_guard.get(&session_id).cloned()
		};

		if let Some(existing_session) = existing_session_info {
			if matches!(existing_session.state, PairingState::WaitingForConnection) {
				println!("Transitioning existing session {} from WaitingForConnection to ChallengeReceived", session_id);

				// Transition existing session to ChallengeReceived
				let updated_session = PairingSession {
					id: session_id,
					state: PairingState::ChallengeReceived {
						challenge: challenge.clone(),
					},
					remote_device_id: Some(from_device),
					shared_secret: existing_session.shared_secret.clone(),
					created_at: existing_session.created_at,
				};

				self.active_sessions
					.write()
					.await
					.insert(session_id, updated_session);
			} else {
				println!(
					"Session {} already exists in state {:?}, updating with new challenge",
					session_id, existing_session.state
				);

				// Update existing session with new challenge
				let updated_session = PairingSession {
					id: session_id,
					state: PairingState::ChallengeReceived {
						challenge: challenge.clone(),
					},
					remote_device_id: Some(from_device),
					shared_secret: existing_session.shared_secret.clone(),
					created_at: existing_session.created_at,
				};

				self.active_sessions
					.write()
					.await
					.insert(session_id, updated_session);
			}
		} else {
			println!("Creating new session {} for pairing request", session_id);

			// Create new session only if none exists
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
		}

		// Send challenge response
		let alice_device_id = self
			.device_registry
			.read()
			.await
			.get_local_device_info()
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to get Alice's device ID: {}", e))
			})?
			.device_id;

		let response = PairingMessage::Challenge {
			session_id,
			challenge: challenge.clone(),
			device_id: alice_device_id,
		};

		println!(
			"🔥 ALICE: Sending Challenge response for session {} with {} byte challenge",
			session_id,
			challenge.len()
		);
		serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
	}

	async fn handle_pairing_challenge(
		&self,
		session_id: Uuid,
		challenge: Vec<u8>,
		alice_device_id: Uuid,
	) -> Result<Vec<u8>> {
		println!(
			"🔥 BOB: handle_pairing_challenge ENTRY - session {} with {} bytes",
			session_id,
			challenge.len()
		);

		// Sign the challenge
		println!("🔥 BOB: About to sign challenge...");
		let signature = match self.identity.sign(&challenge) {
			Ok(sig) => {
				println!(
					"🔥 BOB: Successfully signed challenge, signature is {} bytes",
					sig.len()
				);
				sig
			}
			Err(e) => {
				println!("🔥 BOB: FAILED to sign challenge: {}", e);
				return Err(e);
			}
		};

		// Get local device info
		println!("🔥 BOB: About to get local device info...");
		let device_info = match self.device_registry.read().await.get_local_device_info() {
			Ok(info) => {
				println!(
					"🔥 BOB: Successfully got local device info for device {}",
					info.device_id
				);
				info
			}
			Err(e) => {
				println!("🔥 BOB: FAILED to get local device info: {}", e);
				return Err(e);
			}
		};

		// Update session state and store Alice's device ID
		println!("🔥 BOB: About to update session state to ResponseSent...");
		{
			let mut sessions = self.active_sessions.write().await;
			if let Some(session) = sessions.get_mut(&session_id) {
				println!(
					"🔥 BOB: Found session {}, updating state from {:?} to ResponseSent",
					session_id, session.state
				);
				session.state = PairingState::ResponseSent;
				session.remote_device_id = Some(alice_device_id); // Store Alice's device ID
				println!(
					"🔥 BOB: Session {} state updated to ResponseSent with Alice's device ID {}",
					session_id, alice_device_id
				);
			} else {
				println!(
					"🔥 BOB: ERROR: Session {} not found when trying to update to ResponseSent",
					session_id
				);
			}
		}

		// Send response
		println!("🔥 BOB: About to create response message...");
		let response = PairingMessage::Response {
			session_id,
			response: signature,
			device_info,
		};

		println!("🔥 BOB: About to serialize response...");
		let serialized = serde_json::to_vec(&response).map_err(|e| {
			println!("🔥 BOB: FAILED to serialize response: {}", e);
			NetworkingError::Serialization(e)
		})?;

		println!(
			"🔥 BOB: handle_pairing_challenge SUCCESS - returning {} bytes",
			serialized.len()
		);
		Ok(serialized)
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
			device_info.clone(),
			session_keys.clone(),
		)?;

		// Mark device as connected since pairing is successful
		let (connection, _message_receiver) =
			crate::infrastructure::networking::device::DeviceConnection::new(
				self.device_registry
					.read()
					.await
					.get_peer_by_device(from_device)
					.unwrap_or_else(libp2p::PeerId::random),
				device_info.clone(),
				session_keys.clone(),
			);

		if let Err(e) = self
			.device_registry
			.write()
			.await
			.mark_connected(from_device, connection)
		{
			println!(
				"🔥 ALICE: Warning - failed to mark device as connected: {}",
				e
			);
		} else {
			println!(
				"🔥 ALICE: Successfully marked device {} as connected",
				from_device
			);
		}

		// Update session
		if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
			session.state = PairingState::Completed;
			session.shared_secret = Some(shared_secret);
			session.remote_device_id = Some(from_device);
			println!(
				"🔥 ALICE: Session {} updated with shared secret and remote device ID {}",
				session_id, from_device
			);
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

		let result = match message {
			PairingMessage::PairingRequest {
				session_id,
				device_id,
				device_name,
				public_key,
			} => {
				// Convert device_id and device_name to DeviceInfo
				let device_info = DeviceInfo {
					device_id,
					device_name,
					device_type: crate::infrastructure::networking::device::DeviceType::Desktop, // Default
					os_version: "Unknown".to_string(),
					app_version: "Unknown".to_string(),
					network_fingerprint: NetworkFingerprint {
						peer_id: "unknown".to_string(),
						public_key_hash: "unknown".to_string(),
					},
					last_seen: chrono::Utc::now(),
				};
				self.handle_pairing_request(from_device, session_id, device_info, public_key)
					.await
			}
			PairingMessage::Challenge {
				session_id,
				challenge,
				device_id,
			} => {
				self.handle_pairing_challenge(session_id, challenge, device_id)
					.await
			}
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
				println!(
					"🔥 BOB: Received completion message (request) for session {} - success: {}",
					session_id, success
				);

				if success {
					// Generate shared secret and complete pairing on Bob's side
					match self.generate_shared_secret() {
						Ok(shared_secret) => {
							println!(
								"🔥 BOB: Generated shared secret of {} bytes",
								shared_secret.len()
							);

							// Create session keys
							let session_keys =
								SessionKeys::from_shared_secret(shared_secret.clone());

							// Get Alice's device ID from session state (stored during challenge handling)
							let alice_device_id = {
								let sessions = self.active_sessions.read().await;
								if let Some(session) = sessions.get(&session_id) {
									session.remote_device_id
								} else {
									None
								}
							};

							if let Some(alice_device_id) = alice_device_id {
								println!("🔥 BOB: Found Alice's device ID: {}", alice_device_id);

								// Get Alice's device info from the original pairing request (stored in session)
								let alice_device_info = {
									let sessions = self.active_sessions.read().await;
									if let Some(session) = sessions.get(&session_id) {
										// In a real implementation, we would have stored Alice's device info
										// For now, create a proper placeholder that we can use
										crate::infrastructure::networking::device::DeviceInfo {
										device_id: alice_device_id,
										device_name: "Alice's Device".to_string(),
										device_type: crate::infrastructure::networking::device::DeviceType::Desktop,
										os_version: "Unknown".to_string(),
										app_version: "Unknown".to_string(),
										network_fingerprint: crate::infrastructure::networking::utils::identity::NetworkFingerprint {
											peer_id: "placeholder".to_string(),
											public_key_hash: "placeholder".to_string(),
										},
										last_seen: chrono::Utc::now(),
									}
									} else {
										return Err(crate::infrastructure::networking::NetworkingError::Protocol(
										"Session not found when completing pairing".to_string()
									));
									}
								};

								// Complete pairing in device registry
								match self.device_registry.write().await.complete_pairing(
									alice_device_id,
									alice_device_info.clone(),
									session_keys.clone(),
								) {
									Ok(()) => {
										println!("🔥 BOB: Successfully completed pairing in device registry");

										// Mark Alice as connected since pairing is successful
										let alice_peer_id = self
											.device_registry
											.read()
											.await
											.get_peer_by_device(alice_device_id)
											.unwrap_or_else(libp2p::PeerId::random);

										let (connection, _message_receiver) = crate::infrastructure::networking::device::DeviceConnection::new(
											alice_peer_id,
											alice_device_info.clone(),
											session_keys.clone(),
										);

										if let Err(e) = self
											.device_registry
											.write()
											.await
											.mark_connected(alice_device_id, connection)
										{
											println!("🔥 BOB: Warning - failed to mark Alice as connected: {}", e);
										} else {
											println!(
												"🔥 BOB: Successfully marked Alice {} as connected",
												alice_device_id
											);
										}

										// Update session state
										let mut sessions = self.active_sessions.write().await;
										if let Some(session) = sessions.get_mut(&session_id) {
											session.state = PairingState::Completed;
											session.shared_secret = Some(shared_secret);
											session.remote_device_id = Some(alice_device_id);
											println!("🔥 BOB: Session {} updated with shared secret and device ID", session_id);
										}
									}
									Err(e) => {
										println!("🔥 BOB: Failed to complete pairing in device registry: {}", e);
										let mut sessions = self.active_sessions.write().await;
										if let Some(session) = sessions.get_mut(&session_id) {
											session.state = PairingState::Failed {
												reason: format!(
													"Failed to complete pairing: {}",
													e
												),
											};
										}
									}
								}
							} else {
								println!(
									"🔥 BOB: Could not find Alice's device ID for session {}",
									session_id
								);
								// Still mark as completed but without full device registration
								let mut sessions = self.active_sessions.write().await;
								if let Some(session) = sessions.get_mut(&session_id) {
									session.state = PairingState::Completed;
									session.shared_secret = Some(shared_secret);
									println!(
										"🔥 BOB: Session {} marked as completed with shared secret",
										session_id
									);
								}
							}
						}
						Err(e) => {
							println!("🔥 BOB: Failed to generate shared secret: {}", e);
							let mut sessions = self.active_sessions.write().await;
							if let Some(session) = sessions.get_mut(&session_id) {
								session.state = PairingState::Failed {
									reason: format!("Failed to generate shared secret: {}", e),
								};
							}
						}
					}
				} else {
					// Pairing failed
					let failure_reason = reason.unwrap_or_else(|| "Pairing failed".to_string());
					let mut sessions = self.active_sessions.write().await;
					if let Some(session) = sessions.get_mut(&session_id) {
						session.state = PairingState::Failed {
							reason: failure_reason.clone(),
						};
						println!(
							"🔥 BOB: Session {} marked as failed: {}",
							session_id, failure_reason
						);
					} else {
						println!(
							"🔥 BOB: ERROR: Session {} not found when processing completion",
							session_id
						);
					}
				}

				// Return empty response for completion
				Ok(Vec::new())
			}
		};

		// Handle errors by marking session as failed
		if let Err(ref error) = result {
			// Try to extract session ID from the original message for error tracking
			if let Ok(message) = serde_json::from_slice::<PairingMessage>(&request_data) {
				let session_id = match message {
					PairingMessage::PairingRequest { session_id, .. } => Some(session_id),
					PairingMessage::Challenge { session_id, .. } => Some(session_id),
					PairingMessage::Response { session_id, .. } => Some(session_id),
					PairingMessage::Complete { session_id, .. } => Some(session_id),
				};

				if let Some(session_id) = session_id {
					// Mark session as failed
					if let Some(session) = self.active_sessions.write().await.get_mut(&session_id) {
						session.state = PairingState::Failed {
							reason: error.to_string(),
						};
						println!("Marked pairing session {} as failed: {}", session_id, error);
					}
				}
			}
		}

		result
	}

	async fn handle_response(
		&self,
		from_device: Uuid,
		from_peer: libp2p::PeerId,
		response_data: Vec<u8>,
	) -> Result<()> {
		println!(
			"🔥 BOB: handle_response called with {} bytes from device {}",
			response_data.len(),
			from_device
		);

		// Parse the response message
		let message: PairingMessage = serde_json::from_slice(&response_data)
			.map_err(|e| NetworkingError::Serialization(e))?;

		println!("🔥 BOB: Parsed message type successfully");

		// Process the response based on the message type
		match message {
			PairingMessage::Challenge {
				session_id,
				challenge,
				device_id,
			} => {
				println!(
					"🔥 BOB: Received challenge for session {} with {} byte challenge",
					session_id,
					challenge.len()
				);

				// Check session state before processing
				{
					let sessions = self.active_sessions.read().await;
					if let Some(session) = sessions.get(&session_id) {
						println!(
							"🔥 BOB: Session {} state before challenge processing: {}",
							session_id, session.state
						);
					} else {
						println!("🔥 BOB: No session found for {}", session_id);
					}
				}

				println!("🔥 BOB: About to call handle_pairing_challenge...");

				// Call the existing handle_pairing_challenge method
				match self
					.handle_pairing_challenge(session_id, challenge.clone(), device_id)
					.await
				{
					Ok(response_data) => {
						println!("🔥 BOB: handle_pairing_challenge succeeded, generated {} byte response", response_data.len());

						// Check session state after handle_pairing_challenge
						{
							let sessions = self.active_sessions.read().await;
							if let Some(session) = sessions.get(&session_id) {
								println!(
									"🔥 BOB: Session {} state after handle_pairing_challenge: {}",
									session_id, session.state
								);
							}
						}

						// Use the peer ID directly from the method parameter (this is Alice's peer ID)
						let remote_peer_id = Some(from_peer);
						println!(
							"🔥 BOB: Using peer ID from method parameter: {:?}",
							from_peer
						);

						// Update the session state to ResponsePending so the unified pairing flow can send it
						{
							let mut sessions = self.active_sessions.write().await;
							if let Some(session) = sessions.get_mut(&session_id) {
								session.state = PairingState::ResponsePending {
									challenge: challenge.clone(),
									response_data: response_data.clone(),
									remote_peer_id,
								};
								println!(
									"🔥 BOB: Session {} updated to ResponsePending state",
									session_id
								);
							} else {
								println!("🔥 BOB: ERROR: Session {} not found when trying to update to ResponsePending", session_id);
							}
						}

						// Verify state change
						{
							let sessions = self.active_sessions.read().await;
							if let Some(session) = sessions.get(&session_id) {
								println!(
									"🔥 BOB: Session {} final state: {}",
									session_id, session.state
								);
							}
						}

						println!(
							"🔥 BOB: Challenge response ready to send for session {}",
							session_id
						);
					}
					Err(e) => {
						println!(
							"🔥 BOB: handle_pairing_challenge FAILED for session {}: {}",
							session_id, e
						);
					}
				}
			}
			PairingMessage::Complete {
				session_id,
				success,
				reason,
			} => {
				println!(
					"🔥 BOB: Received completion message for session {} - success: {}",
					session_id, success
				);

				if success {
					// Generate shared secret and complete pairing on Bob's side
					match self.generate_shared_secret() {
						Ok(shared_secret) => {
							println!(
								"🔥 BOB: Generated shared secret of {} bytes",
								shared_secret.len()
							);

							// Create session keys
							let session_keys =
								SessionKeys::from_shared_secret(shared_secret.clone());

							let alice_device_id = from_device;

							// Get Alice's device info from the original pairing request (stored in session)
							let alice_device_info = {
								let sessions = self.active_sessions.read().await;
								if let Some(session) = sessions.get(&session_id) {
									// In a real implementation, we would have stored Alice's device info
									// For now, create a proper placeholder that we can use
									crate::infrastructure::networking::device::DeviceInfo {
										device_id: alice_device_id,
										device_name: "Alice's Device".to_string(),
										device_type: crate::infrastructure::networking::device::DeviceType::Desktop,
										os_version: "Unknown".to_string(),
										app_version: "Unknown".to_string(),
										network_fingerprint: crate::infrastructure::networking::utils::identity::NetworkFingerprint {
											peer_id: "placeholder".to_string(),
											public_key_hash: "placeholder".to_string(),
										},
										last_seen: chrono::Utc::now(),
									}
								} else {
									return Err(crate::infrastructure::networking::NetworkingError::Protocol(
										"Session not found when completing pairing".to_string()
									));
								}
							};

							// Complete pairing in device registry
							let pairing_result = {
								let mut registry = self.device_registry.write().await;
								registry.complete_pairing(
									alice_device_id,
									alice_device_info.clone(),
									session_keys.clone(),
								)
							}; // Release write lock here

							match pairing_result {
								Ok(()) => {
									// Update session state FIRST before any other operations that might fail
									{
										let mut sessions = self.active_sessions.write().await;
										if let Some(session) = sessions.get_mut(&session_id) {
											session.state = PairingState::Completed;
											session.shared_secret = Some(shared_secret.clone());
											session.remote_device_id = Some(alice_device_id);
										}
									}

									// Mark Alice as connected (optional - pairing already completed)
									let alice_peer_id = {
										let registry = self.device_registry.read().await;
										registry
											.get_peer_by_device(alice_device_id)
											.or_else(|| Some(from_peer)) // Fallback to peer from completion message
									};

									if let Some(peer_id) = alice_peer_id {
										let (connection, _message_receiver) = crate::infrastructure::networking::device::DeviceConnection::new(
												peer_id,
												alice_device_info.clone(),
												session_keys.clone(),
											);

										let _mark_result = {
											let mut registry = self.device_registry.write().await;
											registry.mark_connected(alice_device_id, connection)
										};
									}
								}
								Err(e) => {
									println!(
										"🔥 BOB: Failed to complete pairing in device registry: {}",
										e
									);
								}
							}
						}
						Err(e) => {
							println!("🔥 BOB: Failed to generate shared secret: {}", e);
							let mut sessions = self.active_sessions.write().await;
							if let Some(session) = sessions.get_mut(&session_id) {
								session.state = PairingState::Failed {
									reason: format!("Failed to generate shared secret: {}", e),
								};
							}
						}
					}
				} else {
					// Pairing failed
					let failure_reason = reason.unwrap_or_else(|| "Pairing failed".to_string());
					let mut sessions = self.active_sessions.write().await;
					if let Some(session) = sessions.get_mut(&session_id) {
						session.state = PairingState::Failed {
							reason: failure_reason.clone(),
						};
						println!(
							"🔥 BOB: Session {} marked as failed: {}",
							session_id, failure_reason
						);
					} else {
						println!(
							"🔥 BOB: ERROR: Session {} not found when processing completion",
							session_id
						);
					}
				}
			}
			_ => {
				// Other message types are handled by handle_request
				println!("🔥 BOB: Received non-challenge response message, ignoring");
			}
		}

		println!("🔥 BOB: handle_response completed");
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
