//! Volume encryption query output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Encryption information for a specific path
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathEncryptionInfo {
	/// The path that was queried
	pub path: String,

	/// Whether the volume is encrypted
	pub is_encrypted: bool,

	/// Type of encryption (FileVault, BitLocker, LUKS, etc.) if encrypted
	pub encryption_type: Option<String>,

	/// Whether the encrypted volume is currently unlocked
	pub is_unlocked: Option<bool>,

	/// Recommended number of secure delete passes based on encryption and disk type
	pub recommended_passes: u32,

	/// Whether TRIM should be used (for SSDs)
	pub use_trim: bool,

	/// The volume fingerprint this path belongs to
	pub volume_fingerprint: Option<String>,

	/// The volume ID this path belongs to
	pub volume_id: Option<Uuid>,

	/// Human-readable reason for the recommendation
	pub recommendation_reason: String,
}

/// Output for volume encryption query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeEncryptionOutput {
	/// Encryption info for each queried path
	pub paths: Vec<PathEncryptionInfo>,
}
