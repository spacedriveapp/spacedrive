//! Device identity and authentication system
//!
//! Implements persistent network identity integrated with Spacedrive's device system

use chrono::{DateTime, Utc};
use ring::{rand, rand::SecureRandom, signature, signature::KeyPair};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

use crate::device::DeviceManager;
use crate::networking::{NetworkError, Result};

/// Network fingerprint for wire protocol identification
/// This is derived from device UUID + public key for security
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkFingerprint([u8; 32]);

impl NetworkFingerprint {
	/// Create network fingerprint from device UUID and public key
	pub fn from_device(device_id: Uuid, public_key: &PublicKey) -> Self {
		use blake3::Hasher;
		let mut hasher = Hasher::new();
		hasher.update(device_id.as_bytes());
		hasher.update(public_key.as_bytes());
		let hash = hasher.finalize();
		let mut fingerprint = [0u8; 32];
		fingerprint.copy_from_slice(hash.as_bytes());
		NetworkFingerprint(fingerprint)
	}

	/// Get raw bytes
	pub fn as_bytes(&self) -> &[u8; 32] {
		&self.0
	}
}

impl fmt::Display for NetworkFingerprint {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", hex::encode(self.0))
	}
}

/// Ed25519 public key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicKey(Vec<u8>);

/// Ed25519 signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature(Vec<u8>);

impl Signature {
	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.clone()
	}

	pub fn from_bytes(bytes: Vec<u8>) -> Self {
		Signature(bytes)
	}
}

impl PublicKey {
	pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
		if bytes.len() != 32 {
			return Err(NetworkError::EncryptionError(
				"Invalid public key length".to_string(),
			));
		}
		Ok(PublicKey(bytes))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}

	/// Convert to bytes (clone for protocol usage)
	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.clone()
	}

	/// Verify a signature
	pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
		use ring::signature;

		let public_key = signature::UnparsedPublicKey::new(&signature::ED25519, self.as_bytes());
		public_key.verify(data, signature).is_ok()
	}
}

/// Ed25519 private key (encrypted at rest)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedPrivateKey {
	/// Encrypted key material
	pub ciphertext: Vec<u8>,
	/// Salt for key derivation
	pub salt: [u8; 32],
	/// Nonce for encryption
	pub nonce: [u8; 12],
}

/// Ed25519 private key (decrypted in memory)
pub struct PrivateKey {
	key_pair: signature::Ed25519KeyPair,
	pkcs8_bytes: Vec<u8>,
}

impl PrivateKey {
	/// Generate a new Ed25519 key pair
	pub fn generate() -> Result<Self> {
		let rng = rand::SystemRandom::new();
		let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| {
			NetworkError::EncryptionError(format!("Key generation failed: {:?}", e))
		})?;

		let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
			.map_err(|e| NetworkError::EncryptionError(format!("Key parsing failed: {:?}", e)))?;

		Ok(PrivateKey {
			key_pair,
			pkcs8_bytes: pkcs8_bytes.as_ref().to_vec(),
		})
	}

	/// Get public key
	pub fn public_key(&self) -> PublicKey {
		PublicKey(self.key_pair.public_key().as_ref().to_vec())
	}

	/// Sign data
	pub fn sign(&self, data: &[u8]) -> Result<Signature> {
		let signature_bytes = self.key_pair.sign(data).as_ref().to_vec();
		Ok(Signature(signature_bytes))
	}

	/// Encrypt this private key with a password-derived key
	pub fn encrypt(&self, password: &str) -> Result<EncryptedPrivateKey> {
		use ring::{aead, pbkdf2};
		use std::num::NonZeroU32;

		// Generate salt for key derivation
		let mut salt = [0u8; 32];
		let rng = rand::SystemRandom::new();
		rng.fill(&mut salt).map_err(|e| {
			NetworkError::EncryptionError(format!("Random generation failed: {:?}", e))
		})?;

		// Derive key from password
		let iterations = NonZeroU32::new(100_000).unwrap();
		let mut key = [0u8; 32];
		pbkdf2::derive(
			pbkdf2::PBKDF2_HMAC_SHA256,
			iterations,
			&salt,
			password.as_bytes(),
			&mut key,
		);

		// Generate nonce
		let mut nonce = [0u8; 12];
		rng.fill(&mut nonce).map_err(|e| {
			NetworkError::EncryptionError(format!("Random generation failed: {:?}", e))
		})?;

		// Encrypt the private key
		let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key)
			.map_err(|e| NetworkError::EncryptionError(format!("Key creation failed: {:?}", e)))?;
		let sealing_key = aead::LessSafeKey::new(unbound_key);

		// Use the stored PKCS8 bytes from this private key
		let mut ciphertext = self.pkcs8_bytes.clone();
		sealing_key
			.seal_in_place_append_tag(
				aead::Nonce::assume_unique_for_key(nonce),
				aead::Aad::empty(),
				&mut ciphertext,
			)
			.map_err(|e| NetworkError::EncryptionError(format!("Encryption failed: {:?}", e)))?;

		Ok(EncryptedPrivateKey {
			ciphertext,
			salt,
			nonce,
		})
	}

	/// Decrypt an encrypted private key with a password
	pub fn decrypt(encrypted: &EncryptedPrivateKey, password: &str) -> Result<Self> {
		use ring::{aead, pbkdf2};
		use std::num::NonZeroU32;

		// Derive key from password
		let iterations = NonZeroU32::new(100_000).unwrap();
		let mut key = [0u8; 32];
		pbkdf2::derive(
			pbkdf2::PBKDF2_HMAC_SHA256,
			iterations,
			&encrypted.salt,
			password.as_bytes(),
			&mut key,
		);

		// Decrypt the private key
		let unbound_key = aead::UnboundKey::new(&aead::AES_256_GCM, &key)
			.map_err(|e| NetworkError::EncryptionError(format!("Key creation failed: {:?}", e)))?;
		let opening_key = aead::LessSafeKey::new(unbound_key);

		let mut ciphertext = encrypted.ciphertext.clone();
		let plaintext = opening_key
			.open_in_place(
				aead::Nonce::assume_unique_for_key(encrypted.nonce),
				aead::Aad::empty(),
				&mut ciphertext,
			)
			.map_err(|e| NetworkError::EncryptionError(format!("Decryption failed: {:?}", e)))?;

		let key_pair = signature::Ed25519KeyPair::from_pkcs8(plaintext)
			.map_err(|e| NetworkError::EncryptionError(format!("Key parsing failed: {:?}", e)))?;

		Ok(PrivateKey {
			key_pair,
			pkcs8_bytes: plaintext.to_vec(),
		})
	}
}

/// Network identity tied to persistent device identity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkIdentity {
	/// MUST match the persistent device UUID from DeviceManager
	pub device_id: Uuid,

	/// Device's public key (Ed25519) - STORED PERSISTENTLY
	pub public_key: PublicKey,

	/// Device's private key (encrypted at rest) - STORED PERSISTENTLY
	pub(crate) private_key: EncryptedPrivateKey,

	/// Human-readable device name (from DeviceConfig)
	pub device_name: String,

	/// Network-specific identifier (derived from device_id + public_key)
	pub network_fingerprint: NetworkFingerprint,
}

impl NetworkIdentity {
	/// Create a new network identity for testing/demo purposes
	/// WARNING: This generates new keys and is NOT suitable for production
	pub fn new_temporary(device_id: Uuid, device_name: String, password: &str) -> Result<Self> {
		// Generate new keys (NOT production-ready)
		let private_key = PrivateKey::generate()?;
		let public_key = private_key.public_key();
		let encrypted_private_key = private_key.encrypt(password)?;
		let network_fingerprint = NetworkFingerprint::from_device(device_id, &public_key);

		Ok(Self {
			device_id,
			public_key,
			private_key: encrypted_private_key,
			device_name,
			network_fingerprint,
		})
	}

	/// Create network identity from existing device configuration
	pub async fn from_device_manager(
		device_manager: &DeviceManager,
		password: &str,
	) -> Result<Self> {
		let device_config = device_manager.config().map_err(|e| {
			NetworkError::AuthenticationFailed(format!("Failed to get device config: {}", e))
		})?;

		// Try to load existing network keys
		if let Ok(Some(keys)) = Self::load_network_keys(&device_config.id, password) {
			let network_fingerprint =
				NetworkFingerprint::from_device(device_config.id, &keys.public_key);
			return Ok(Self {
				device_id: device_config.id,
				public_key: keys.public_key,
				private_key: keys.encrypted_private_key,
				device_name: device_config.name,
				network_fingerprint,
			});
		}

		// Generate new network keys if none exist
		let private_key = PrivateKey::generate()?;
		let public_key = private_key.public_key();
		let encrypted_private_key = private_key.encrypt(password)?;
		let network_fingerprint = NetworkFingerprint::from_device(device_config.id, &public_key);

		// Save keys persistently
		Self::save_network_keys(
			&device_config.id,
			&public_key,
			&encrypted_private_key,
			password,
		)?;

		Ok(Self {
			device_id: device_config.id,
			public_key,
			private_key: encrypted_private_key,
			device_name: device_config.name,
			network_fingerprint,
		})
	}

	/// Load network keys from device-specific storage
	fn load_network_keys(device_id: &Uuid, password: &str) -> Result<Option<EncryptedNetworkKeys>> {
		let path = Self::network_keys_path(device_id)?;

		if !path.exists() {
			return Ok(None);
		}

		let content =
			std::fs::read_to_string(&path).map_err(|e| NetworkError::IoError(e.to_string()))?;

		let keys: EncryptedNetworkKeys = serde_json::from_str(&content).map_err(|e| {
			NetworkError::SerializationError(format!("Failed to parse network keys: {}", e))
		})?;

		// Verify we can decrypt with the provided password
		let _test_key = PrivateKey::decrypt(&keys.encrypted_private_key, password)?;

		Ok(Some(keys))
	}

	/// Save network keys to device-specific storage
	fn save_network_keys(
		device_id: &Uuid,
		public_key: &PublicKey,
		private_key: &EncryptedPrivateKey,
		password: &str,
	) -> Result<()> {
		let path = Self::network_keys_path(device_id)?;

		// Ensure parent directory exists
		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent).map_err(|e| NetworkError::IoError(e.to_string()))?;
		}

		let keys = EncryptedNetworkKeys {
			encrypted_private_key: private_key.clone(),
			public_key: public_key.clone(),
			salt: private_key.salt,
			created_at: Utc::now(),
		};

		let content = serde_json::to_string_pretty(&keys).map_err(|e| {
			NetworkError::SerializationError(format!("Failed to serialize network keys: {}", e))
		})?;

		std::fs::write(&path, content).map_err(|e| NetworkError::IoError(e.to_string()))?;

		tracing::info!("Network keys saved for device {}", device_id);
		Ok(())
	}

	/// Get the path for storing network keys
	fn network_keys_path(device_id: &Uuid) -> Result<PathBuf> {
		let data_dir = crate::config::default_data_dir()
			.map_err(|e| NetworkError::TransportError(format!("Failed to get data dir: {}", e)))?;

		Ok(data_dir.join("network_keys.json"))
	}

	/// Unlock the private key with password
	pub fn unlock_private_key(&self, password: &str) -> Result<PrivateKey> {
		PrivateKey::decrypt(&self.private_key, password)
	}

	/// Verify a signature
	pub fn verify_signature(&self, data: &[u8], signature: &[u8]) -> bool {
		use ring::signature;

		let public_key =
			signature::UnparsedPublicKey::new(&signature::ED25519, self.public_key.as_bytes());
		public_key.verify(data, signature).is_ok()
	}

	/// Create DeviceInfo from this identity
	pub fn to_device_info(&self) -> DeviceInfo {
		DeviceInfo::new(
			self.device_id,
			self.device_name.clone(),
			self.public_key.clone(),
		)
	}
}

/// Network keys stored persistently
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedNetworkKeys {
	/// Ed25519 private key encrypted with user password
	pub encrypted_private_key: EncryptedPrivateKey,

	/// Public key (not encrypted)
	pub public_key: PublicKey,

	/// Salt for key derivation
	pub salt: [u8; 32],

	/// When these keys were created
	pub created_at: DateTime<Utc>,
}

/// Master key for device management
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterKey {
	/// User's master password derives this
	key_encryption_key: [u8; 32],

	/// Encrypted with key_encryption_key - NOW USES PERSISTENT DEVICE UUIDs
	device_private_keys: HashMap<Uuid, EncryptedPrivateKey>,
}

impl MasterKey {
	/// Create a new master key from password
	pub fn new(password: &str) -> Result<Self> {
		use ring::pbkdf2;
		use std::num::NonZeroU32;

		// Generate salt for master key derivation
		let mut salt = [0u8; 32];
		let rng = rand::SystemRandom::new();
		rng.fill(&mut salt).map_err(|e| {
			NetworkError::EncryptionError(format!("Random generation failed: {:?}", e))
		})?;

		// Derive master key from password
		let iterations = NonZeroU32::new(100_000).unwrap();
		let mut key = [0u8; 32];
		pbkdf2::derive(
			pbkdf2::PBKDF2_HMAC_SHA256,
			iterations,
			&salt,
			password.as_bytes(),
			&mut key,
		);

		Ok(MasterKey {
			key_encryption_key: key,
			device_private_keys: HashMap::new(),
		})
	}

	/// Add a device to the master key
	pub fn add_device(&mut self, device_id: Uuid, private_key: EncryptedPrivateKey) {
		self.device_private_keys.insert(device_id, private_key);
	}

	/// Remove a device from the master key
	pub fn remove_device(&mut self, device_id: &Uuid) -> bool {
		self.device_private_keys.remove(device_id).is_some()
	}

	/// Get all managed device IDs
	pub fn device_ids(&self) -> Vec<Uuid> {
		self.device_private_keys.keys().cloned().collect()
	}
}

/// Device information for remote devices
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
	/// Persistent device UUID
	pub device_id: Uuid,

	/// Human-readable device name
	pub device_name: String,

	/// Network public key
	pub public_key: PublicKey,

	/// Network fingerprint for wire protocol
	pub network_fingerprint: NetworkFingerprint,

	/// Last time this device was seen
	pub last_seen: DateTime<Utc>,
}

impl DeviceInfo {
	pub fn new(device_id: Uuid, device_name: String, public_key: PublicKey) -> Self {
		let network_fingerprint = NetworkFingerprint::from_device(device_id, &public_key);
		Self {
			device_id,
			device_name,
			public_key,
			network_fingerprint,
			last_seen: Utc::now(),
		}
	}
}
