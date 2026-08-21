//! Ephemeral source registry
//!
//! A source is a registered root that owns its own ephemeral index partition
//! and snapshot: a volume, an external drive, or an explicitly indexed tree.
//! Registrations persist in a JSON file in the snapshot cache directory so a
//! source keeps its identity — and can restore its snapshot — across launches
//! and across remounts at different mount points.
//!
//! This registry is deliberately lightweight; it converges with the archive
//! sources registry when filesystem sources gain durable spine stores.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
	fs,
	path::{Path, PathBuf},
};
use uuid::Uuid;

/// Registry file format version.
const REGISTRY_VERSION: u32 = 1;

/// A registered ephemeral source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
	/// Stable identity for the source; keys the snapshot file.
	pub id: Uuid,
	/// Root path at last attach. Remounts may change this; the fingerprint is
	/// the durable identity for removable volumes.
	pub root: PathBuf,
	/// Volume fingerprint for removable sources, when known. Lets a drive be
	/// recognized as the same source when it returns at a different mount point.
	pub fingerprint: Option<String>,
	/// Unix seconds at registration.
	pub created_at_secs: u64,
	/// Unix seconds at last successful attach or index.
	pub last_seen_secs: u64,
	/// Entry count at last snapshot, so the UI can show it without restoring.
	#[serde(default)]
	pub entry_count: Option<u64>,
	/// Total file bytes at last snapshot.
	#[serde(default)]
	pub total_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
	version: u32,
	sources: Vec<SourceRecord>,
}

/// Persistent set of source registrations.
#[derive(Debug)]
pub struct SourceRegistry {
	/// Where registrations are written, or `None` for a session-only registry.
	/// Sources still work in that mode; nothing about them outlives the process.
	path: Option<PathBuf>,
	sources: Vec<SourceRecord>,
}

fn now_secs() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0)
}

impl SourceRegistry {
	/// Load the registry from `dir/sources.json`, or start empty.
	pub fn load(dir: &Path) -> Self {
		let path = dir.join("sources.json");
		let sources = fs::read(&path)
			.ok()
			.and_then(|bytes| serde_json::from_slice::<RegistryFile>(&bytes).ok())
			.filter(|file| file.version == REGISTRY_VERSION)
			.map(|file| file.sources)
			.unwrap_or_default();
		Self {
			path: Some(path),
			sources,
		}
	}

	/// A registry that lives only as long as the process.
	///
	/// Used when the cache is built without a sources directory. Writes are not
	/// attempted rather than attempted and ignored, so a failure from `save` in
	/// the persistent case always means something is actually wrong.
	pub fn in_memory() -> Self {
		Self {
			path: None,
			sources: Vec::new(),
		}
	}

	fn save(&self) -> Result<()> {
		let Some(path) = &self.path else {
			return Ok(());
		};
		let file = RegistryFile {
			version: REGISTRY_VERSION,
			sources: self.sources.clone(),
		};
		let bytes = serde_json::to_vec_pretty(&file).context("serialize source registry")?;
		let tmp = path.with_extension("json.tmp");
		fs::write(&tmp, bytes).context("write source registry")?;
		fs::rename(&tmp, path).context("rename source registry")?;
		Ok(())
	}

	/// Register a root as a source, or refresh an existing registration.
	///
	/// Matching order: fingerprint (a returning drive, possibly at a new mount
	/// point) first, then exact root, and the root arm is only reachable when
	/// the caller has no fingerprint to offer or the stored record has none
	/// either. A mount point is where a drive happens to be, not what it is, so
	/// a fingerprint that matches nothing registers a new source rather than
	/// adopting whatever last occupied that path — otherwise a different drive
	/// inherits the previous one's id, its snapshot, and anything keyed to it.
	pub fn register(&mut self, root: &Path, fingerprint: Option<String>) -> Result<SourceRecord> {
		let now = now_secs();

		if let Some(fp) = fingerprint.as_deref() {
			if let Some(record) = self
				.sources
				.iter_mut()
				.find(|s| s.fingerprint.as_deref() == Some(fp))
			{
				record.root = root.to_path_buf();
				record.last_seen_secs = now;
				let record = record.clone();
				self.save()?;
				return Ok(record);
			}
		}

		let root_match = self.sources.iter_mut().find(|s| {
			s.root == root
				&& match (s.fingerprint.as_deref(), fingerprint.as_deref()) {
					// Both identified and not equal: the fingerprint arm above
					// already declined, so this is a different drive.
					(Some(stored), Some(incoming)) => stored == incoming,
					// Adopting a record that was registered before fingerprints
					// were available is the one case worth naming the drive for.
					(None, _) => true,
					// The caller cannot identify what it mounted; the stored
					// record can. Do not let an unidentified mount claim it.
					(Some(_), None) => false,
				}
		});

		if let Some(record) = root_match {
			if record.fingerprint.is_none() {
				record.fingerprint = fingerprint;
			}
			record.last_seen_secs = now;
			let record = record.clone();
			self.save()?;
			return Ok(record);
		}

		let record = SourceRecord {
			id: Uuid::now_v7(),
			root: root.to_path_buf(),
			fingerprint,
			created_at_secs: now,
			last_seen_secs: now,
			entry_count: None,
			total_bytes: None,
		};
		self.sources.push(record.clone());
		self.save()?;
		Ok(record)
	}

	/// All registered sources.
	pub fn all(&self) -> &[SourceRecord] {
		&self.sources
	}

	/// The registered source whose root is the longest prefix of `path`.
	///
	/// Two records can share a root: a drive that fails fingerprint matching
	/// registers a new source at the mount point the previous one used. The most
	/// recently seen of those is the one currently mounted there, so it wins.
	pub fn resolve(&self, path: &Path) -> Option<&SourceRecord> {
		self.sources
			.iter()
			.filter(|s| path.starts_with(&s.root))
			.max_by_key(|s| (s.root.as_os_str().len(), s.last_seen_secs))
	}

	/// Look up a source by id.
	pub fn by_id(&self, id: Uuid) -> Option<&SourceRecord> {
		self.sources.iter().find(|s| s.id == id)
	}

	/// Record the entry count and byte total for a source (written alongside
	/// snapshots, so listings show sizes without loading anything).
	pub fn update_stats(&mut self, id: Uuid, entry_count: u64, total_bytes: u64) -> Result<()> {
		if let Some(record) = self.sources.iter_mut().find(|s| s.id == id) {
			record.entry_count = Some(entry_count);
			record.total_bytes = Some(total_bytes);
			record.last_seen_secs = now_secs();
			self.save()?;
		}
		Ok(())
	}

	/// Remove a source registration. The caller owns deleting the snapshot.
	pub fn remove(&mut self, id: Uuid) -> Result<Option<SourceRecord>> {
		let Some(idx) = self.sources.iter().position(|s| s.id == id) else {
			return Ok(None);
		};
		let record = self.sources.remove(idx);
		self.save()?;
		Ok(Some(record))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn register_resolve_and_fingerprint_rebind() {
		let dir = tempfile::tempdir().unwrap();
		let mut reg = SourceRegistry::load(dir.path());

		let a = reg
			.register(Path::new("/Volumes/Archive"), Some("fp-1".into()))
			.unwrap();
		let b = reg.register(Path::new("/Users/me"), None).unwrap();
		assert_ne!(a.id, b.id);

		// Longest prefix wins.
		let hit = reg
			.resolve(Path::new("/Volumes/Archive/photos/x.jpg"))
			.unwrap();
		assert_eq!(hit.id, a.id);
		assert!(reg.resolve(Path::new("/tmp/elsewhere")).is_none());

		// Same drive back at a different mount point keeps its identity.
		let again = reg
			.register(Path::new("/Volumes/Archive 1"), Some("fp-1".into()))
			.unwrap();
		assert_eq!(again.id, a.id);
		assert_eq!(again.root, PathBuf::from("/Volumes/Archive 1"));

		// A different drive at a mount point the first one used is a different
		// source: it must not inherit an id, and must not overwrite the stored
		// fingerprint of the drive that is merely absent.
		let other = reg
			.register(Path::new("/Volumes/Archive 1"), Some("fp-2".into()))
			.unwrap();
		assert_ne!(other.id, a.id);
		assert_eq!(
			reg.by_id(a.id).unwrap().fingerprint.as_deref(),
			Some("fp-1"),
			"the absent drive kept its own fingerprint"
		);

		// Two records now share a root; the most recently seen wins resolution.
		assert_eq!(
			reg.resolve(Path::new("/Volumes/Archive 1/x.jpg"))
				.unwrap()
				.id,
			other.id
		);

		// An unidentified mount must not claim a record that is identified.
		let anonymous = reg.register(Path::new("/Volumes/Archive 1"), None).unwrap();
		assert_ne!(anonymous.id, other.id);

		// Registrations survive reload: the two originals plus the two drives
		// that declined to adopt an existing record.
		let reloaded = SourceRegistry::load(dir.path());
		assert_eq!(reloaded.all().len(), 4);
		assert_eq!(
			reloaded.by_id(a.id).unwrap().root,
			PathBuf::from("/Volumes/Archive 1")
		);
	}
}
