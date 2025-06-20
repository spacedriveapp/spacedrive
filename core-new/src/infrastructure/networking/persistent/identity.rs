//! Enhanced network identity with persistent device relationships
//!
//! Extends the base NetworkIdentity to support persistent device pairing, session keys,
//! and connection management for long-lived device relationships.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use super::storage::{EncryptedData, SecureStorage};
use crate::device::DeviceManager;
use crate::networking::{DeviceInfo, NetworkError, NetworkIdentity, PublicKey, Result};

/// Enhanced network identity with device relationships
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentNetworkIdentity {
	/// Core network identity (unchanged)
	pub identity: NetworkIdentity,

	/// Paired devices with trust levels
	pub paired_devices: HashMap<Uuid, PairedDeviceRecord>,

	/// Active connection sessions
	pub active_sessions: HashMap<Uuid, ActiveSession>,

	/// Connection history and metrics
	pub connection_history: Vec<ConnectionRecord>,

	/// Last updated timestamp
	pub updated_at: DateTime<Utc>,

	/// Storage version for migration compatibility
	pub version: u32,
}

/// Record of a paired device
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedDeviceRecord {
	/// Device information from pairing
	pub device_info: DeviceInfo,

	/// When this device was first paired
	pub paired_at: DateTime<Utc>,

	/// Last successful connection
	pub last_connected: Option<DateTime<Utc>>,

	/// Trust level for this device
	pub trust_level: TrustLevel,

	/// Long-term session keys for this device
	pub session_keys: Option<EncryptedSessionKeys>,

	/// Connection preferences
	pub connection_config: ConnectionConfig,

	/// Whether to auto-connect to this device
	pub auto_connect: bool,

	/// Number of successful connections
	pub connection_count: u64,

	/// Number of failed connection attempts
	pub failed_attempts: u64,

	/// Last known network addresses
	pub last_addresses: Vec<String>,
}

/// Trust levels for paired devices
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TrustLevel {
	/// Full trust - auto-connect, file sharing enabled
	Trusted,

	/// Verified trust - manual approval required for sensitive operations
	Verified,

	/// Expired trust - require re-pairing
	Expired,

	/// Revoked - never connect
	Revoked,
}

/// Session keys encrypted with device relationship key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedSessionKeys {
	/// Encrypted session keys for this device relationship
	pub encrypted_data: EncryptedData,

	/// When these keys were generated
	pub created_at: DateTime<Utc>,

	/// Key rotation schedule
	pub expires_at: DateTime<Utc>,

	/// Key generation version
	pub key_version: u32,
}

/// Raw session keys for device communication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionKeys {
	/// Key for sending data to remote device
	pub send_key: [u8; 32],
	/// Key for receiving data from remote device
	pub receive_key: [u8; 32],
	/// Key for message authentication
	pub mac_key: [u8; 32],
	/// Session identifier
	pub session_id: Uuid,
	/// When these keys were created
	pub created_at: DateTime<Utc>,
}

/// Connection configuration for a device
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionConfig {
	/// Preferred transport order
	pub preferred_transports: Vec<TransportType>,

	/// Known addresses for this device
	pub known_addresses: Vec<String>,

	/// Connection retry policy
	pub retry_policy: RetryPolicy,

	/// Keep-alive interval
	pub keepalive_interval_secs: u64,

	/// Connection timeout in seconds
	pub connection_timeout_secs: u64,

	/// Maximum concurrent connections
	pub max_concurrent_connections: u32,
}

/// Transport types for device connections
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransportType {
	Tcp,
	Quic,
	WebSocket,
	WebRtc,
}

/// Connection retry policy
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
	/// Maximum number of retry attempts
	pub max_attempts: u32,
	/// Base delay between retries in seconds
	pub base_delay_secs: u64,
	/// Maximum delay between retries in seconds
	pub max_delay_secs: u64,
	/// Exponential backoff multiplier
	pub backoff_multiplier: f64,
}

/// Active session information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveSession {
	/// Session identifier
	pub session_id: Uuid,
	/// Current session keys
	pub session_keys: SessionKeys,
	/// When session was established
	pub established_at: DateTime<Utc>,
	/// Last activity timestamp
	pub last_activity: DateTime<Utc>,
	/// Session state
	pub state: SessionState,
}

/// Session states
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
	Establishing,
	Active,
	Refreshing,
	Expired,
	Closed,
}

/// Connection history record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionRecord {
	/// Remote device ID
	pub device_id: Uuid,
	/// When connection was established
	pub connected_at: DateTime<Utc>,
	/// When connection was closed
	pub disconnected_at: Option<DateTime<Utc>>,
	/// Connection duration in seconds
	pub duration_secs: Option<u64>,
	/// Connection result
	pub result: ConnectionResult,
	/// Remote addresses used
	pub remote_addresses: Vec<String>,
	/// Transport type used
	pub transport: TransportType,
	/// Data transferred (bytes)
	pub bytes_sent: u64,
	pub bytes_received: u64,
}

/// Connection results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConnectionResult {
	Success,
	Failed(String),
	Timeout,
	AuthenticationFailed,
	NetworkError(String),
}

impl Default for ConnectionConfig {
	fn default() -> Self {
		Self {
			preferred_transports: vec![TransportType::Quic, TransportType::Tcp],
			known_addresses: Vec::new(),
			retry_policy: RetryPolicy::default(),
			keepalive_interval_secs: 30,
			connection_timeout_secs: 30,
			max_concurrent_connections: 1,
		}
	}
}

impl Default for RetryPolicy {
	fn default() -> Self {
		Self {
			max_attempts: 5,
			base_delay_secs: 1,
			max_delay_secs: 60,
			backoff_multiplier: 2.0,
		}
	}
}

impl SessionKeys {
	/// Generate new session keys
	pub fn new() -> Self {
		use ring::rand::{SecureRandom, SystemRandom};

		let rng = SystemRandom::new();
		let mut send_key = [0u8; 32];
		let mut receive_key = [0u8; 32];
		let mut mac_key = [0u8; 32];

		// Generate cryptographically secure random keys
		rng.fill(&mut send_key)
			.expect("Failed to generate send key");
		rng.fill(&mut receive_key)
			.expect("Failed to generate receive key");
		rng.fill(&mut mac_key).expect("Failed to generate MAC key");

		Self {
			send_key,
			receive_key,
			mac_key,
			session_id: Uuid::new_v4(),
			created_at: Utc::now(),
		}
	}

	/// Generate ephemeral session keys from existing keys
	pub fn generate_ephemeral(device_id: &Uuid, base_keys: &SessionKeys) -> Result<Self> {
		use blake3::Hasher;

		// Derive new keys using HKDF-like construction
		let mut hasher = Hasher::new();
		hasher.update(b"spacedrive-ephemeral-keys-v1");
		hasher.update(device_id.as_bytes());
		hasher.update(&base_keys.send_key);
		hasher.update(&base_keys.receive_key);
		hasher.update(&base_keys.mac_key);
		hasher.update(&Utc::now().timestamp().to_le_bytes());

		let derived = hasher.finalize();
		let key_material = derived.as_bytes();

		let mut send_key = [0u8; 32];
		let mut receive_key = [0u8; 32];
		let mut mac_key = [0u8; 32];

		send_key.copy_from_slice(&key_material[0..32]);
		receive_key.copy_from_slice(&key_material[32..64]);

		// Generate MAC key with different input
		let mut mac_hasher = Hasher::new();
		mac_hasher.update(b"spacedrive-mac-key-v1");
		mac_hasher.update(&send_key);
		mac_hasher.update(&receive_key);
		let mac_derived = mac_hasher.finalize();
		mac_key.copy_from_slice(&mac_derived.as_bytes()[0..32]);

		Ok(Self {
			send_key,
			receive_key,
			mac_key,
			session_id: Uuid::new_v4(),
			created_at: Utc::now(),
		})
	}

	/// Check if keys need rotation based on age
	pub fn needs_rotation(&self, rotation_interval: Duration) -> bool {
		Utc::now().signed_duration_since(self.created_at) > rotation_interval
	}
}

impl PersistentNetworkIdentity {
	/// Load or create persistent network identity
	pub async fn load_or_create(device_manager: &DeviceManager, password: &str) -> Result<Self> {
		let device_config = device_manager.config().map_err(|e| {
			NetworkError::AuthenticationFailed(format!("Failed to get device config: {}", e))
		})?;

		let data_dir = crate::config::default_data_dir()
			.map_err(|e| NetworkError::TransportError(format!("Failed to get data dir: {}", e)))?;

		let storage = SecureStorage::new(data_dir.join("network"));
		let storage_path = storage.device_identity_path(&device_config.id);

		if let Some(identity) = storage.load::<Self>(&storage_path, password).await? {
			tracing::info!(
				"Loaded persistent network identity for device {}",
				device_config.id
			);
			return Ok(identity);
		}

		// Create new persistent identity
		Self::create_new(device_manager, password).await
	}

	/// Create new persistent identity
	async fn create_new(device_manager: &DeviceManager, password: &str) -> Result<Self> {
		// Create base network identity
		let identity = NetworkIdentity::from_device_manager(device_manager, password).await?;

		let persistent_identity = Self {
			identity,
			paired_devices: HashMap::new(),
			active_sessions: HashMap::new(),
			connection_history: Vec::new(),
			updated_at: Utc::now(),
			version: 1,
		};

		// Save to disk
		persistent_identity.save(password).await?;

		tracing::info!("Created new persistent network identity");
		Ok(persistent_identity)
	}

	/// Save identity to encrypted storage
	pub async fn save(&self, password: &str) -> Result<()> {
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| NetworkError::TransportError(format!("Failed to get data dir: {}", e)))?;

		let storage = SecureStorage::new(data_dir.join("network"));
		let storage_path = storage.device_identity_path(&self.identity.device_id);

		storage.store(&storage_path, self, password).await?;

		tracing::debug!(
			"Saved persistent network identity for device {}",
			self.identity.device_id
		);
		Ok(())
	}

	/// Add a newly paired device
	pub fn add_paired_device(
		&mut self,
		device_info: DeviceInfo,
		session_keys: SessionKeys,
		password: &str,
	) -> Result<()> {
		let device_id = device_info.device_id;

		// Encrypt session keys for storage
		let encrypted_keys = self.encrypt_session_keys(&session_keys, password)?;

		// Create device record
		let device_record = PairedDeviceRecord {
			device_info,
			paired_at: Utc::now(),
			last_connected: None,
			trust_level: TrustLevel::Trusted,
			session_keys: Some(encrypted_keys),
			connection_config: ConnectionConfig::default(),
			auto_connect: true,
			connection_count: 0,
			failed_attempts: 0,
			last_addresses: Vec::new(),
		};

		// Store in identity
		self.paired_devices.insert(device_id, device_record);
		self.updated_at = Utc::now();

		tracing::info!("Added paired device: {}", device_id);
		Ok(())
	}

	/// Remove a paired device
	pub fn remove_paired_device(&mut self, device_id: &Uuid) -> bool {
		let removed = self.paired_devices.remove(device_id).is_some();
		if removed {
			// Also remove active session
			self.active_sessions.remove(device_id);
			self.updated_at = Utc::now();
			tracing::info!("Removed paired device: {}", device_id);
		}
		removed
	}

	/// Update device trust level
	pub fn update_trust_level(&mut self, device_id: &Uuid, trust_level: TrustLevel) -> Result<()> {
		if let Some(record) = self.paired_devices.get_mut(device_id) {
			record.trust_level = trust_level;
			self.updated_at = Utc::now();
			tracing::info!(
				"Updated trust level for device {}: {:?}",
				device_id,
				record.trust_level
			);
			Ok(())
		} else {
			Err(NetworkError::DeviceNotFound(*device_id))
		}
	}

	/// Get all trusted devices
	pub fn trusted_devices(&self) -> Vec<&PairedDeviceRecord> {
		self.paired_devices
			.values()
			.filter(|record| record.trust_level == TrustLevel::Trusted)
			.collect()
	}

	/// Get devices that should auto-connect
	pub fn auto_connect_devices(&self) -> Vec<PairedDeviceRecord> {
		self.paired_devices
			.values()
			.filter(|record| {
				record.auto_connect
					&& matches!(
						record.trust_level,
						TrustLevel::Trusted | TrustLevel::Verified
					)
			})
			.cloned()
			.collect()
	}

	/// Record successful connection
	pub fn record_connection_success(&mut self, device_id: &Uuid, addresses: Vec<String>) {
		if let Some(record) = self.paired_devices.get_mut(device_id) {
			record.last_connected = Some(Utc::now());
			record.connection_count += 1;
			record.failed_attempts = 0; // Reset failed attempts on success
			record.last_addresses = addresses;
			self.updated_at = Utc::now();
		}
	}

	/// Record failed connection attempt
	pub fn record_connection_failure(&mut self, device_id: &Uuid) {
		if let Some(record) = self.paired_devices.get_mut(device_id) {
			record.failed_attempts += 1;
			self.updated_at = Utc::now();

			// Auto-expire devices with too many failed attempts
			if record.failed_attempts > 10 {
				record.trust_level = TrustLevel::Expired;
				tracing::warn!(
					"Device {} marked as expired due to too many failed connections",
					device_id
				);
			}
		}
	}

	/// Add connection history entry
	pub fn add_connection_record(&mut self, record: ConnectionRecord) {
		self.connection_history.push(record);
		self.updated_at = Utc::now();

		// Keep only recent history
		const MAX_HISTORY: usize = 1000;
		if self.connection_history.len() > MAX_HISTORY {
			self.connection_history
				.drain(0..self.connection_history.len() - MAX_HISTORY);
		}
	}

	/// Encrypt session keys with device-specific password
	fn encrypt_session_keys(
		&self,
		keys: &SessionKeys,
		password: &str,
	) -> Result<EncryptedSessionKeys> {
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| NetworkError::TransportError(format!("Failed to get data dir: {}", e)))?;

		let storage = SecureStorage::new(data_dir);
		let json_data = serde_json::to_vec(keys).map_err(|e| {
			NetworkError::SerializationError(format!("Failed to serialize session keys: {}", e))
		})?;

		let encrypted_data = storage.encrypt_data(&json_data, password)?;

		Ok(EncryptedSessionKeys {
			encrypted_data,
			created_at: Utc::now(),
			expires_at: Utc::now() + Duration::days(30), // 30-day expiration
			key_version: 1,
		})
	}

	/// Decrypt session keys
	pub fn decrypt_session_keys(
		&self,
		encrypted: &EncryptedSessionKeys,
		password: &str,
	) -> Result<SessionKeys> {
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| NetworkError::TransportError(format!("Failed to get data dir: {}", e)))?;

		let storage = SecureStorage::new(data_dir);
		let decrypted_data = storage.decrypt_data(&encrypted.encrypted_data, password)?;

		let keys: SessionKeys = serde_json::from_slice(&decrypted_data).map_err(|e| {
			NetworkError::SerializationError(format!("Failed to deserialize session keys: {}", e))
		})?;

		Ok(keys)
	}

	/// Clean up expired sessions and old history
	pub fn cleanup_expired_data(&mut self) {
		let now = Utc::now();

		// Remove expired sessions
		self.active_sessions.retain(|_, session| {
			!matches!(session.state, SessionState::Expired | SessionState::Closed)
		});

		// Mark devices with expired session keys
		for record in self.paired_devices.values_mut() {
			if let Some(session_keys) = &record.session_keys {
				if now > session_keys.expires_at {
					// Don't automatically expire trusted devices, just mark keys as needing refresh
					if record.trust_level != TrustLevel::Trusted {
						record.trust_level = TrustLevel::Expired;
					}
				}
			}
		}

		// Keep only recent connection history (last 90 days)
		let cutoff = now - Duration::days(90);
		self.connection_history
			.retain(|record| record.connected_at > cutoff);

		self.updated_at = now;
	}
}
