//! Source listing output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A registered source of either kind.
///
/// `data_type` is what forks them: `filesystem` for a walked root, the
/// adapter's data type otherwise. The fields an adapter has no answer for are
/// optional rather than defaulted, so a client can tell "no root" from "root
/// unknown".
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceInfo {
	pub id: Uuid,
	pub name: String,
	/// `filesystem`, or the adapter's data type.
	pub data_type: String,
	/// Absent for a filesystem source: a walk has no adapter.
	pub adapter_id: Option<String>,
	/// Records the source holds, from its last completed pass.
	pub item_count: i64,
	pub last_synced: Option<String>,
	pub status: String,

	/// Absolute root as currently mounted. Filesystem sources only.
	pub root: Option<String>,
	/// The medium underneath, when Spacedrive tracks one.
	pub volume_uuid: Option<Uuid>,
	/// Whether the origin answers right now. Always true for an adapter, whose
	/// origin is a network service rather than a drive in a drawer.
	pub attached: bool,
	pub total_bytes: Option<i64>,
	/// Last time the origin answered. Absent for an adapter, whose registry
	/// tracks a sync cursor rather than an attachment.
	pub last_seen_at: Option<String>,
}

impl SourceInfo {
	/// A source backed by an adapter, from the archive registry.
	pub fn adapter(
		id: Uuid,
		name: String,
		data_type: String,
		adapter_id: String,
		item_count: i64,
		last_synced: Option<String>,
		status: String,
	) -> Self {
		Self {
			id,
			name,
			data_type,
			adapter_id: Some(adapter_id),
			item_count,
			last_synced,
			status,
			root: None,
			volume_uuid: None,
			attached: true,
			total_bytes: None,
			last_seen_at: None,
		}
	}
}
