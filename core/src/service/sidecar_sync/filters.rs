use crate::ops::sidecar::{SidecarFormat, SidecarKind, SidecarVariant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Filters for sidecar sync operations
#[derive(Debug, Clone, Serialize, Deserialize, Default, Type)]
pub struct SidecarSyncFilters {
	/// Filter by specific sidecar kinds (e.g., only thumbs)
	pub kinds: Option<Vec<SidecarKind>>,

	/// Filter by specific content UUIDs
	pub content_uuids: Option<Vec<Uuid>>,

	/// Maximum number of sidecars to sync in this batch
	pub max_count: Option<usize>,

	/// Cursor for pagination (format: "content_uuid|kind|variant")
	pub cursor: Option<String>,
}

/// Sync mode for sidecar operations
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SidecarSyncMode {
	/// Download sidecars we don't have
	PullMissing,
	/// Upload sidecars to devices that need them (future)
	PushNew,
	/// Both pull and push (future)
	Bidirectional,
}

/// A sidecar that exists in the database but isn't available locally
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingSidecar {
	pub sidecar_uuid: Uuid,
	pub content_uuid: Uuid,
	pub kind: SidecarKind,
	pub variant: SidecarVariant,
	pub format: SidecarFormat,
	pub size: i64,
	pub checksum: Option<String>,
}

/// Information about a device that has a sidecar
#[derive(Debug, Clone)]
pub struct SidecarSource {
	pub device_uuid: Uuid,
	pub last_seen_at: DateTime<Utc>,
	pub verified_checksum: Option<String>,
}

/// A planned sidecar transfer with source device selected
#[derive(Debug, Clone)]
pub struct SidecarTransferPlan {
	pub sidecar: MissingSidecar,
	pub source_device: Uuid,
}
