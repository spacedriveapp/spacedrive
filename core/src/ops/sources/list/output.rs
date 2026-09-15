//! Source listing output

use crate::ops::indexing::ephemeral::SourceRecord;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
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
	/// The paired device this source was replicated from. Absent for a
	/// source this library registered itself; a replica is read-only here
	/// and its registry ops run on the owning device.
	pub device_id: Option<Uuid>,
	/// The owning device's display name, for a replica.
	pub device_label: Option<String>,
}

impl SourceInfo {
	/// One registration, whatever fills it.
	///
	/// `mount_point` is where the source's volume is mounted right now, for the
	/// kind that has one. A filesystem source stores its root relative to its
	/// volume, so without the mount there is no absolute path to report and the
	/// source reads as detached, which is what a drive in a drawer is.
	pub fn from_row(
		row: crate::infra::db::entities::source::Model,
		mount_point: Option<&Path>,
	) -> Self {
		let adapter_id = row.adapter_id.clone();
		let data_type = row.data_type.clone();
		let status = row.status.clone();
		let last_seen_at = Some(row.last_seen_at.to_rfc3339());
		let last_synced = row.last_indexed_at.map(|at| at.to_rfc3339());
		let item_count = row.record_count.unwrap_or(0);
		let total_bytes = row.total_bytes;
		let record = SourceRecord::from_row(row, mount_point);

		// An adapter's origin is a network service rather than a drive, so it
		// is attached in the only sense the word has here.
		let attached = if adapter_id.is_some() {
			true
		} else {
			record.root.exists()
		};

		Self {
			id: record.id,
			name: record.name,
			data_type,
			adapter_id,
			item_count,
			last_synced,
			status,
			root: (!record.root.as_os_str().is_empty())
				.then(|| record.root.to_string_lossy().into_owned()),
			volume_uuid: record.volume_uuid,
			attached,
			total_bytes,
			last_seen_at,
			device_id: None,
			device_label: None,
		}
	}

	/// A paired device's source, replicated through the peer-mount plane.
	/// The metadata is the replica's own: counts and bytes from the owner's
	/// listing, `last_synced` from when the snapshot arrived here.
	pub fn from_remote_share(share: &crate::service::mounts::peer::RemoteShare) -> Self {
		let root = &share.info.root;
		let name = root
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| share.info.id.to_string());
		let synced_at =
			chrono::DateTime::<chrono::Utc>::from_timestamp(share.synced_at_secs as i64, 0)
				.map(|at| at.to_rfc3339());

		Self {
			id: share.info.id,
			name,
			data_type: "filesystem".to_string(),
			adapter_id: None,
			item_count: share.info.entry_count.unwrap_or(0) as i64,
			last_synced: synced_at.clone(),
			status: "replica".to_string(),
			root: Some(root.to_string_lossy().into_owned()),
			volume_uuid: share.info.volume_uuid,
			attached: share.info.attached,
			total_bytes: share.info.total_bytes.map(|bytes| bytes as i64),
			last_seen_at: synced_at,
			device_id: Some(share.device_id),
			device_label: Some(share.device_label.clone()),
		}
	}
}
