//! Source listing output

use crate::ops::indexing::SourceRecord;
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
	/// How the drive under a filesystem source stands: mounted, unmounted,
	/// or locked because its encryption key is not loaded. Absent for an
	/// adapter, a replica, or a source on media Spacedrive does not track.
	#[serde(default)]
	pub volume_state: Option<crate::volume::VolumeState>,
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
	/// The fetch bringing this replica up to date, while one is in flight.
	/// Present on a replica being refreshed and on a source whose first
	/// copy has not finished, which has no other row to appear in.
	#[serde(default)]
	pub transfer: Option<crate::service::mounts::replication::ReplicaTransferProgress>,
	/// Where the catalog lives. Absent for an adapter source and a replica,
	/// whose stores sit in their own layouts.
	#[serde(default)]
	pub placement: Option<crate::ops::indexing::sources::StorePlacement>,
	/// The catalog's directory on this machine, when this machine has it.
	#[serde(default)]
	pub store_path: Option<String>,
	/// The settings the source was saved with, so a re-add can show what it
	/// keeps. Absent for an adapter source and a replica, like `placement`.
	#[serde(default)]
	pub settings: Option<crate::ops::indexing::sources::SourceConfig>,
	/// The library's offline copy of a store placed on its source: whether
	/// one exists, whether reads answer from it right now, when it was
	/// taken and how far behind the origin it is. Absent unless the
	/// catalog lives on the source.
	#[serde(default)]
	pub offline_copy: Option<crate::service::mounts::offline::OfflineCopyInfo>,
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
		let placement = adapter_id.is_none().then_some(record.config.placement);
		let settings = adapter_id.is_none().then(|| record.config.clone());

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
			volume_state: None,
			total_bytes,
			last_seen_at,
			device_id: None,
			device_label: None,
			transfer: None,
			placement,
			store_path: None,
			settings,
			offline_copy: None,
		}
	}

	/// A replica known from a persisted manifest whose arena is not loaded.
	/// The owner's last-synced counts still describe it; what is missing is
	/// a local artifact to browse, which `status` says plainly.
	pub fn from_replica_manifest(
		device_id: Uuid,
		device_label: &str,
		entry: &crate::service::mounts::peer::ReplicaEntry,
	) -> Self {
		let root = &entry.info.root;
		let name = root
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| entry.info.id.to_string());
		let synced_at =
			chrono::DateTime::<chrono::Utc>::from_timestamp(entry.synced_at_secs as i64, 0)
				.map(|at| at.to_rfc3339());

		Self {
			id: entry.info.id,
			name,
			data_type: "filesystem".to_string(),
			adapter_id: None,
			item_count: entry.info.entry_count.unwrap_or(0) as i64,
			last_synced: synced_at.clone(),
			status: "replica_unavailable".to_string(),
			root: Some(root.to_string_lossy().into_owned()),
			volume_uuid: entry.info.volume_uuid,
			attached: false,
			volume_state: entry.info.volume_state,
			total_bytes: entry.info.total_bytes.map(|bytes| bytes as i64),
			last_seen_at: synced_at,
			device_id: Some(device_id),
			device_label: Some(device_label.to_string()),
			transfer: crate::service::mounts::replication::transfer(entry.info.id),
			placement: None,
			store_path: None,
			settings: None,
			offline_copy: None,
		}
	}

	/// A source whose first replica is still arriving. Nothing else lists it
	/// yet: there is no share to browse and no manifest entry until the
	/// artifact validates, but the transfer itself is worth seeing.
	pub fn from_transfer(
		transfer: &crate::service::mounts::replication::ReplicaTransferProgress,
	) -> Self {
		let name = transfer
			.root
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| transfer.source_id.to_string());
		Self {
			id: transfer.source_id,
			name,
			data_type: "filesystem".to_string(),
			adapter_id: None,
			item_count: 0,
			last_synced: None,
			status: "replica_fetching".to_string(),
			root: Some(transfer.root.to_string_lossy().into_owned()),
			volume_uuid: None,
			attached: false,
			volume_state: None,
			total_bytes: None,
			last_seen_at: None,
			device_id: Some(transfer.device_id),
			device_label: Some(transfer.device_label.clone()),
			transfer: Some(transfer.clone()),
			placement: None,
			store_path: None,
			settings: None,
			offline_copy: None,
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
			volume_state: share.info.volume_state,
			total_bytes: share.info.total_bytes.map(|bytes| bytes as i64),
			last_seen_at: synced_at,
			device_id: Some(share.device_id),
			device_label: Some(share.device_label.clone()),
			transfer: crate::service::mounts::replication::transfer(share.info.id),
			placement: None,
			store_path: None,
			settings: None,
			offline_copy: None,
		}
	}
}
