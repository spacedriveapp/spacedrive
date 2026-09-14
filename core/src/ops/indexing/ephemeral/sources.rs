//! The source registry: which roots this library indexes, and what drive each
//! one sits on.
//!
//! Registrations live in `library.db` (`entities::source`). This module holds
//! the in-memory half: the matching rules, longest-prefix path resolution, and
//! the record shape the cache keys its partitions on. It performs no I/O, so
//! the arena's read path never waits on a database to answer "which source owns
//! this path".
//!
//! ## Volume-anchored roots
//!
//! A source that sits on a tracked volume stores its root *relative to that
//! volume*, and its absolute root is that path joined onto wherever the volume
//! is mounted right now. A whole-drive source has an empty relative root.
//!
//! This is what replaced fingerprint matching. The registry used to carry a
//! volume fingerprint so a returning drive could be recognised at a new mount
//! point, which duplicated an identity the volume manager already maintains.
//! Anchoring to `volume_uuid` instead makes a remount cost nothing: the volume
//! keeps its identity, the source keeps its relative root, and the absolute
//! path re-derives. A drive that comes back somewhere else is no longer a case
//! anything has to handle.
//!
//! Sources with no volume, which is ordinary for network shares and for roots
//! on media Spacedrive does not track, keep an absolute root and match on it.

use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::infra::db::entities::source;

/// The drive a source is being registered against.
#[derive(Debug, Clone)]
pub struct VolumeAnchor {
	pub uuid: Uuid,
	/// Where the volume is mounted right now.
	pub mount_point: PathBuf,
}

/// A registered source, as the cache and the UI see it.
#[derive(Debug, Clone)]
pub struct SourceRecord {
	/// Stable identity. Keys the source's directory under `SourceDirs`.
	pub id: Uuid,
	pub name: String,
	/// Absolute root as currently mounted, or empty when the anchoring volume
	/// is not attached and the source therefore has no location on this machine
	/// right now. Derived, so it changes across remounts while the record does
	/// not. See [`SourceRecord::is_locatable`].
	pub root: PathBuf,
	/// The durable half of the root: a path within the volume, or the absolute
	/// path when there is no volume. Empty for a whole-drive source.
	pub relative_root: String,
	pub volume_uuid: Option<Uuid>,
	pub created_at: DateTime<Utc>,
	pub last_seen_at: DateTime<Utc>,
	/// Records at last snapshot, so a listing can show a count without
	/// restoring anything.
	pub record_count: Option<u64>,
	pub directory_count: Option<u64>,
	pub total_bytes: Option<u64>,
	/// Distinct sets of bytes, which is fewer than `record_count` wherever the
	/// source holds the same file twice.
	pub content_count: Option<u64>,
	/// Bytes remaining if every within-source duplicate collapsed to one copy.
	pub unique_bytes: Option<u64>,
}

/// Which volume index a path belongs to.
///
/// A partition is a property of the drive rather than of a registration: one
/// drive, one map, whatever is persisted off it. Keying it by source id instead
/// is what made a source nested inside another fork the index rather than
/// narrow it, down to minting a second uuid for the same file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VolumeKey {
	/// A volume Spacedrive has a record for, which is every mount it can see,
	/// network shares included.
	Id(Uuid),
	/// A filesystem with no volume record, identified by where it begins. Rare
	/// enough to be a fallback rather than a second way of doing this.
	Path(PathBuf),
	/// Paths on nothing tracked at all.
	Scratch,
}

/// Namespace for volume index ids. Changing it renames every index's directory
/// on disk, so it is pinned by a test.
const VOLUME_INDEX_NAMESPACE: Uuid = Uuid::from_u128(0x9f2c_4e77_1b3a_4d58_9c21_6f0e_7a84_bd39);

impl VolumeKey {
	/// A stable id, so an index can own a directory without a registry row of
	/// its own. Derived rather than minted: the same drive is the same index on
	/// every run.
	pub fn id(&self) -> Uuid {
		match self {
			Self::Id(uuid) => Uuid::new_v5(&VOLUME_INDEX_NAMESPACE, uuid.as_bytes()),
			Self::Path(path) => {
				Uuid::new_v5(&VOLUME_INDEX_NAMESPACE, path.to_string_lossy().as_bytes())
			}
			Self::Scratch => Uuid::nil(),
		}
	}
}

impl SourceRecord {
	/// Rebuild from a stored row, resolving the absolute root against wherever
	/// the anchoring volume is mounted now.
	///
	/// A row whose volume is absent resolves to its relative root on its own,
	/// which is the drive-in-a-drawer case: the record stands, its snapshot
	/// still restores, and nothing on disk answers.
	pub fn from_row(row: source::Model, mount_point: Option<&Path>) -> Self {
		let relative_root = row.root.unwrap_or_default();
		let root = match (row.volume_uuid, mount_point) {
			(Some(_), Some(mount)) => join_relative(mount, &relative_root),
			_ => PathBuf::from(&relative_root),
		};
		Self {
			id: row.uuid,
			name: row.name,
			root,
			relative_root,
			volume_uuid: row.volume_uuid,
			created_at: row.created_at,
			last_seen_at: row.last_seen_at,
			record_count: row.record_count.map(|c| c.max(0) as u64),
			directory_count: row.directory_count.map(|c| c.max(0) as u64),
			total_bytes: row.total_bytes.map(|b| b.max(0) as u64),
			content_count: row.content_count.map(|c| c.max(0) as u64),
			unique_bytes: row.unique_bytes.map(|b| b.max(0) as u64),
		}
	}
}

impl SourceRecord {
	/// Whether this source has an absolute path on this machine right now.
	///
	/// A volume-anchored source whose volume is absent has no root to give, and
	/// an empty path is not a location: `Path::starts_with` accepts it for
	/// every path on the system, so a record left in that state would capture
	/// every lookup that nothing else claimed. That is how a drive in a drawer
	/// came to own the browse of an unrelated folder, and then had its index
	/// replaced by that folder's contents.
	pub fn is_locatable(&self) -> bool {
		!self.root.as_os_str().is_empty()
	}
}

fn join_relative(mount_point: &Path, relative: &str) -> PathBuf {
	if relative.is_empty() {
		mount_point.to_path_buf()
	} else {
		mount_point.join(relative)
	}
}

/// The relative root a path implies within a volume, or `None` when the path
/// is not under that mount point at all.
fn relative_to(mount_point: &Path, root: &Path) -> Option<String> {
	let relative = root.strip_prefix(mount_point).ok()?;
	Some(
		relative
			.to_string_lossy()
			.replace(std::path::MAIN_SEPARATOR, "/"),
	)
}

fn display_name(root: &Path) -> String {
	root.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| root.to_string_lossy().into_owned())
}

/// The set of registered sources, in memory.
///
/// Pure: every mutator returns the record that changed, and the caller writes
/// it. Persistence belongs to whoever holds the library, which this does not.
#[derive(Debug, Default)]
pub struct SourceRegistry {
	sources: Vec<SourceRecord>,
}

impl SourceRegistry {
	/// Seed from stored rows. `mount_point_of` resolves a volume uuid to where
	/// that volume is mounted now, and returns `None` for a volume that is not
	/// attached.
	pub fn from_rows(
		rows: Vec<source::Model>,
		mount_point_of: impl Fn(Uuid) -> Option<PathBuf>,
	) -> Self {
		let sources = rows
			.into_iter()
			.map(|row| {
				let mount = row.volume_uuid.and_then(&mount_point_of);
				SourceRecord::from_row(row, mount.as_deref())
			})
			.collect();
		Self { sources }
	}

	/// Register a root, or refresh an existing registration.
	///
	/// Matching is by anchor: a volume-backed source is the same source when it
	/// is the same volume and the same path within it, whatever the mount point
	/// happens to be today. An unanchored source matches on its absolute root.
	pub fn register(&mut self, root: &Path, volume: Option<&VolumeAnchor>) -> SourceRecord {
		let now = Utc::now();

		let relative_root = match volume {
			// A root that is not under the mount point it claims to be on is a
			// caller error, and anchoring it anyway would bind the source to a
			// volume that cannot produce its path. Fall back to unanchored.
			Some(anchor) => relative_to(&anchor.mount_point, root),
			None => None,
		};

		let anchor = relative_root.as_ref().and(volume);
		let key = relative_root
			.clone()
			.unwrap_or_else(|| root.to_string_lossy().into_owned());

		let existing = self.sources.iter_mut().find(|source| {
			source.volume_uuid == anchor.map(|a| a.uuid) && source.relative_root == key
		});

		if let Some(record) = existing {
			record.root = root.to_path_buf();
			record.last_seen_at = now;
			return record.clone();
		}

		let record = SourceRecord {
			id: Uuid::now_v7(),
			name: display_name(root),
			root: root.to_path_buf(),
			relative_root: key,
			volume_uuid: anchor.map(|a| a.uuid),
			created_at: now,
			last_seen_at: now,
			record_count: None,
			directory_count: None,
			total_bytes: None,
			content_count: None,
			unique_bytes: None,
		};
		self.sources.push(record.clone());
		record
	}

	/// The drive a source sits on, and where it begins.
	///
	/// A tracked volume answers this wherever one is anchored. Without one the
	/// outermost registered root standing over this source is, which is what
	/// makes a source inside a network share share the share's map rather than
	/// starting a second one. Registration order does not matter: adding an
	/// outer source later moves the inner one onto its drive, because this is
	/// asked of the registry rather than remembered on the record.
	pub fn volume_of(&self, record: &SourceRecord) -> (VolumeKey, PathBuf) {
		if let Some(uuid) = record.volume_uuid {
			let mount = self
				.sources
				.iter()
				.filter(|source| source.volume_uuid == Some(uuid) && source.is_locatable())
				.map(|source| source.root.clone())
				.min_by_key(|root| root.as_os_str().len())
				.unwrap_or_else(|| record.root.clone());
			return (VolumeKey::Id(uuid), mount);
		}

		let outermost = self
			.sources
			.iter()
			.filter(|source| {
				source.volume_uuid.is_none()
					&& source.is_locatable()
					&& record.root.starts_with(&source.root)
			})
			.map(|source| source.root.clone())
			.min_by_key(|root| root.as_os_str().len())
			.unwrap_or_else(|| record.root.clone());

		(VolumeKey::Path(outermost.clone()), outermost)
	}

	/// All registered sources.
	pub fn all(&self) -> &[SourceRecord] {
		&self.sources
	}

	/// The registered source whose root is the longest prefix of `path`.
	///
	/// Two records can share a root when a volume is replaced by another at the
	/// same mount point, so the most recently seen of those wins.
	pub fn resolve(&self, path: &Path) -> Option<&SourceRecord> {
		self.sources
			.iter()
			.filter(|source| source.is_locatable() && path.starts_with(&source.root))
			.max_by_key(|source| (source.root.as_os_str().len(), source.last_seen_at))
	}

	/// Look up a source by id.
	pub fn by_id(&self, id: Uuid) -> Option<&SourceRecord> {
		self.sources.iter().find(|source| source.id == id)
	}

	/// Point every source anchored to a volume at that volume's new mount
	/// point, returning what changed so the caller can persist it.
	///
	/// Roots also resolve at library attach, which covers a drive that moved
	/// while the daemon was down. This covers one that moves while it is up,
	/// and it wants a `VolumeEvent::VolumeMountChanged` subscriber, which
	/// nothing has yet.
	pub fn remount(&mut self, volume_uuid: Uuid, mount_point: &Path) -> Vec<SourceRecord> {
		let now = Utc::now();
		self.sources
			.iter_mut()
			.filter(|source| source.volume_uuid == Some(volume_uuid))
			.map(|source| {
				source.root = join_relative(mount_point, &source.relative_root);
				source.last_seen_at = now;
				source.clone()
			})
			.collect()
	}

	/// Record what the last snapshot held, so listings can show a size without
	/// loading one.
	pub fn update_stats(
		&mut self,
		id: Uuid,
		counts: crate::ops::indexing::ephemeral::SourceCounts,
	) -> Option<SourceRecord> {
		let record = self.sources.iter_mut().find(|source| source.id == id)?;
		record.record_count = Some(counts.records);
		record.directory_count = Some(counts.directories);
		record.total_bytes = Some(counts.bytes);
		record.content_count = Some(counts.contents);
		record.unique_bytes = Some(counts.unique_bytes);
		record.last_seen_at = Utc::now();
		Some(record.clone())
	}

	/// Drop a registration. The caller owns deleting the source's directory.
	pub fn remove(&mut self, id: Uuid) -> Option<SourceRecord> {
		let index = self.sources.iter().position(|source| source.id == id)?;
		Some(self.sources.remove(index))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn anchor(mount: &str) -> VolumeAnchor {
		VolumeAnchor {
			uuid: Uuid::from_u128(1),
			mount_point: PathBuf::from(mount),
		}
	}

	#[test]
	fn a_remount_keeps_the_source() {
		let mut registry = SourceRegistry::default();
		let drive = anchor("/Volumes/Archive");
		let first = registry.register(Path::new("/Volumes/Archive"), Some(&drive));
		assert_eq!(
			first.relative_root, "",
			"a whole drive has no path within itself"
		);

		// The same drive comes back one mount point over. The volume manager
		// kept its uuid, so the source needs no evidence of its own.
		let moved = anchor("/Volumes/Archive 1");
		let again = registry.register(Path::new("/Volumes/Archive 1"), Some(&moved));
		assert_eq!(again.id, first.id);
		assert_eq!(again.root, PathBuf::from("/Volumes/Archive 1"));
		assert_eq!(registry.all().len(), 1);
	}

	#[test]
	fn a_different_drive_at_the_same_mount_point_is_a_different_source() {
		let mut registry = SourceRegistry::default();
		let first = registry.register(
			Path::new("/Volumes/Archive"),
			Some(&anchor("/Volumes/Archive")),
		);

		let other = VolumeAnchor {
			uuid: Uuid::from_u128(2),
			mount_point: PathBuf::from("/Volumes/Archive"),
		};
		let second = registry.register(Path::new("/Volumes/Archive"), Some(&other));

		assert_ne!(
			second.id, first.id,
			"a mount point is where a drive is, not what it is"
		);
		assert_eq!(registry.all().len(), 2);
	}

	#[test]
	fn folders_on_one_drive_are_distinct_sources() {
		let mut registry = SourceRegistry::default();
		let drive = anchor("/Volumes/Archive");
		let photos = registry.register(Path::new("/Volumes/Archive/Photos"), Some(&drive));
		let video = registry.register(Path::new("/Volumes/Archive/Video"), Some(&drive));

		assert_ne!(photos.id, video.id);
		assert_eq!(photos.relative_root, "Photos");
		assert_eq!(video.relative_root, "Video");

		// Both follow the drive without being re-registered.
		let moved = registry.remount(drive.uuid, Path::new("/Volumes/Archive 1"));
		assert_eq!(moved.len(), 2);
		assert_eq!(
			registry.by_id(photos.id).unwrap().root,
			PathBuf::from("/Volumes/Archive 1/Photos")
		);
	}

	/// A drive in a drawer owns nothing on this machine.
	///
	/// Its record resolves to an empty root, and an empty path is a prefix of
	/// every path, so without this it claims every lookup nothing else does.
	/// Downstream that means an unrelated browse lands in the detached
	/// source's partition and its snapshot is saved from those contents.
	#[test]
	fn a_source_whose_volume_is_absent_claims_nothing() {
		let row = source::Model {
			id: 1,
			uuid: Uuid::from_u128(9),
			name: "Archive".to_string(),
			data_type: source::FILESYSTEM_DATA_TYPE.to_string(),
			adapter_id: None,
			config: "{}".to_string(),
			// A whole-drive source: its path within the volume is empty, so
			// with no mount point there is nothing to join it onto.
			root: Some(String::new()),
			volume_uuid: Some(Uuid::from_u128(1)),
			record_count: Some(2_000_000),
			directory_count: None,
			total_bytes: None,
			content_count: None,
			unique_bytes: None,
			last_indexed_at: None,
			status: "idle".to_string(),
			trust_tier: "authored".to_string(),
			created_at: Utc::now(),
			last_seen_at: Utc::now(),
		};

		let registry = SourceRegistry::from_rows(vec![row], |_| None);
		let record = &registry.all()[0];
		assert!(!record.is_locatable());
		assert_eq!(
			record.record_count,
			Some(2_000_000),
			"the record still stands"
		);

		assert!(registry.resolve(Path::new("/Users/me/Desktop")).is_none());
		assert!(registry.resolve(Path::new("/anything/at/all")).is_none());
	}

	#[test]
	fn longest_prefix_wins_resolution() {
		let mut registry = SourceRegistry::default();
		let drive = anchor("/Volumes/Archive");
		let whole = registry.register(Path::new("/Volumes/Archive"), Some(&drive));
		let photos = registry.register(Path::new("/Volumes/Archive/Photos"), Some(&drive));

		assert_eq!(
			registry
				.resolve(Path::new("/Volumes/Archive/Photos/x.jpg"))
				.unwrap()
				.id,
			photos.id
		);
		assert_eq!(
			registry
				.resolve(Path::new("/Volumes/Archive/Video/x.mov"))
				.unwrap()
				.id,
			whole.id
		);
		assert!(registry.resolve(Path::new("/elsewhere")).is_none());
	}

	#[test]
	fn a_root_outside_its_claimed_volume_is_not_anchored() {
		let mut registry = SourceRegistry::default();
		// Anchoring this would bind the source to a volume that cannot produce
		// its path, so the record stands on its absolute root instead.
		let record = registry.register(
			Path::new("/Users/me/Notes"),
			Some(&anchor("/Volumes/Archive")),
		);

		assert!(record.volume_uuid.is_none());
		assert_eq!(record.relative_root, "/Users/me/Notes");
	}

	#[test]
	fn an_unattached_volume_leaves_the_record_standing() {
		let row = source::Model {
			id: 1,
			uuid: Uuid::from_u128(9),
			name: "Archive".to_string(),
			data_type: source::FILESYSTEM_DATA_TYPE.to_string(),
			adapter_id: None,
			config: "{}".to_string(),
			root: Some("Photos".to_string()),
			volume_uuid: Some(Uuid::from_u128(1)),
			record_count: Some(12),
			directory_count: None,
			total_bytes: Some(4096),
			content_count: None,
			unique_bytes: None,
			last_indexed_at: None,
			status: "idle".to_string(),
			trust_tier: "authored".to_string(),
			created_at: Utc::now(),
			last_seen_at: Utc::now(),
		};

		let registry = SourceRegistry::from_rows(vec![row], |_| None);
		let record = &registry.all()[0];

		// The drive is in a drawer. The record and its counts survive; only the
		// absolute path is unavailable, which is exactly what is true.
		assert_eq!(record.record_count, Some(12));
		assert_eq!(record.relative_root, "Photos");
	}
}
