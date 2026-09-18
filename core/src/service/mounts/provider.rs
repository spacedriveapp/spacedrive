//! The byte plane: one trait over every place a mounted file's bytes live.
//!
//! A frontend resolves a target to a provider and then asks for ranges.
//! Nothing above this module knows whether the bytes come off a local disk,
//! out of a cloud volume, or over the byterange protocol from a paired
//! device, and no frontend re-implements range handling or the mapping from
//! failure to a status the user can read.
//!
//! The block cache lands here as a decorator over `Arc<dyn ByteProvider>`,
//! which is why the resolver hands back a trait object rather than a
//! concrete type.

use crate::context::CoreContext;
use crate::domain::volume::VolumeFingerprint;
use crate::ops::indexing::volume_index::SourceStatus;
use crate::volume::VolumeBackend;
use async_trait::async_trait;
use bytes::Bytes;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;

/// Largest range one call satisfies, per class. Local reads stay small
/// because they cost a memcpy; remote reads amortise a round trip.
const LOCAL_READ_CHUNK: u64 = 256 * 1024;
const REMOTE_READ_CHUNK: u64 = 4 * 1024 * 1024;

/// A file within a source, addressed the way the arena addresses it: a real
/// path for physical roots, a URI for cloud roots.
#[derive(Debug, Clone)]
pub struct ByteTarget {
	pub source_id: Uuid,
	pub path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub struct ByteStat {
	pub size: u64,
	pub modified: Option<SystemTime>,
}

/// Drives read sizes now and cache and read-ahead policy later; a local
/// disk, a cloud volume and a LAN peer do not want the same numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderClass {
	Local,
	Cloud,
	Peer,
}

/// Everything a frontend needs in order to answer. Kept apart from the HTTP
/// layer so the mapping from failure to status code exists in one place.
#[derive(Debug)]
pub enum ByteError {
	NotFound(String),
	/// The source is registered but its bytes are not reachable from here.
	Detached {
		source: PathBuf,
	},
	PeerUnavailable {
		device: String,
		reason: String,
	},
	/// A cloud source whose volume or backend cannot serve reads.
	NoBackend(String),
	/// The source cannot be served through this plane at all.
	Unsupported(String),
	Io(String),
}

impl std::fmt::Display for ByteError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotFound(msg) if msg.is_empty() => write!(f, "not found"),
			Self::NotFound(msg) => write!(f, "not found: {msg}"),
			Self::Detached { source } => {
				write!(f, "source detached: {}", source.display())
			}
			Self::PeerUnavailable { device, reason } => {
				write!(f, "peer {device} unavailable: {reason}")
			}
			Self::NoBackend(msg) | Self::Unsupported(msg) | Self::Io(msg) => write!(f, "{msg}"),
		}
	}
}

#[async_trait]
pub trait ByteProvider: Send + Sync {
	async fn stat(&self, target: &ByteTarget) -> Result<ByteStat, ByteError>;

	/// Bytes for `range`. A short result means end of file; callers loop
	/// until they have what they asked for or get nothing back.
	async fn read_range(&self, target: &ByteTarget, range: Range<u64>) -> Result<Bytes, ByteError>;

	fn class(&self) -> ProviderClass;

	/// Largest range a single `read_range` will satisfy.
	fn max_read(&self) -> u64 {
		match self.class() {
			ProviderClass::Local => LOCAL_READ_CHUNK,
			ProviderClass::Cloud | ProviderClass::Peer => REMOTE_READ_CHUNK,
		}
	}
}

// ------------------------------------------------------------------ local

/// Bytes off a real filesystem. The handle is opened once by the resolver
/// and read positionally, so concurrent ranges against one file never share
/// a seek cursor.
pub struct LocalProvider {
	file: Arc<std::fs::File>,
	stat: ByteStat,
}

impl LocalProvider {
	pub async fn open(path: &Path) -> Result<Self, ByteError> {
		let path = path.to_path_buf();
		tokio::task::spawn_blocking(move || {
			let file = std::fs::File::open(&path).map_err(|err| match err.kind() {
				std::io::ErrorKind::NotFound => ByteError::NotFound(String::new()),
				_ => ByteError::Io(err.to_string()),
			})?;
			let meta = file.metadata().map_err(|e| ByteError::Io(e.to_string()))?;
			Ok(Self {
				stat: ByteStat {
					size: meta.len(),
					modified: meta.modified().ok(),
				},
				file: Arc::new(file),
			})
		})
		.await
		.map_err(|e| ByteError::Io(e.to_string()))?
	}
}

fn read_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
	#[cfg(unix)]
	{
		use std::os::unix::fs::FileExt;
		file.read_at(buf, offset)
	}
	#[cfg(windows)]
	{
		use std::os::windows::fs::FileExt;
		file.seek_read(buf, offset)
	}
}

#[async_trait]
impl ByteProvider for LocalProvider {
	async fn stat(&self, _target: &ByteTarget) -> Result<ByteStat, ByteError> {
		Ok(self.stat)
	}

	async fn read_range(
		&self,
		_target: &ByteTarget,
		range: Range<u64>,
	) -> Result<Bytes, ByteError> {
		let want = range.end.saturating_sub(range.start);
		if want == 0 {
			return Ok(Bytes::new());
		}
		let file = self.file.clone();
		tokio::task::spawn_blocking(move || {
			let mut buf = vec![0u8; want as usize];
			let mut filled = 0usize;
			while filled < buf.len() {
				match read_at(&file, &mut buf[filled..], range.start + filled as u64) {
					Ok(0) => break,
					Ok(n) => filled += n,
					Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
					Err(err) => return Err(ByteError::Io(err.to_string())),
				}
			}
			buf.truncate(filled);
			Ok(Bytes::from(buf))
		})
		.await
		.map_err(|e| ByteError::Io(e.to_string()))?
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Local
	}
}

// ------------------------------------------------------------------ cloud

/// Bytes through a volume backend. Arena paths for cloud sources carry the
/// full URI while the backend operator is rooted at the bucket, so the key
/// is resolved once here rather than at every read.
pub struct CloudProvider {
	backend: Arc<dyn VolumeBackend>,
	key: PathBuf,
}

#[async_trait]
impl ByteProvider for CloudProvider {
	async fn stat(&self, _target: &ByteTarget) -> Result<ByteStat, ByteError> {
		match self.backend.metadata(&self.key).await {
			Ok(meta) => Ok(ByteStat {
				size: meta.size,
				modified: meta.modified,
			}),
			Err(err) => Err(ByteError::NotFound(err.to_string())),
		}
	}

	async fn read_range(
		&self,
		_target: &ByteTarget,
		range: Range<u64>,
	) -> Result<Bytes, ByteError> {
		self.backend
			.read_range(&self.key, range)
			.await
			.map_err(|err| ByteError::Io(err.to_string()))
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Cloud
	}
}

// ------------------------------------------------------------------- peer

/// Bytes over the byterange protocol from a paired device. Metadata comes
/// from the replicated snapshot, so only `open`-time reads cross the wire.
pub struct PeerProvider {
	context: Arc<CoreContext>,
	device_id: Uuid,
	device_label: String,
}

#[async_trait]
impl ByteProvider for PeerProvider {
	async fn stat(&self, target: &ByteTarget) -> Result<ByteStat, ByteError> {
		match super::peer::stat(&self.context, self.device_id, target.path.clone()).await {
			Ok((size, modified)) => Ok(ByteStat { size, modified }),
			Err(err) => Err(ByteError::PeerUnavailable {
				device: self.device_label.clone(),
				reason: err.to_string(),
			}),
		}
	}

	async fn read_range(&self, target: &ByteTarget, range: Range<u64>) -> Result<Bytes, ByteError> {
		let want = range.end.saturating_sub(range.start);
		super::peer::read_range(
			&self.context,
			self.device_id,
			target.path.clone(),
			range.start,
			want,
		)
		.await
		.map_err(|err| ByteError::Io(err.to_string()))
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Peer
	}
}

// --------------------------------------------------------------- resolver

/// What a frontend has resolved a request to: a local source's subtree or a
/// replicated peer source's subtree, plus the path inside it.
pub enum MountTarget {
	Local(SourceStatus, PathBuf),
	Remote(Arc<super::peer::RemoteShare>, PathBuf),
}

impl MountTarget {
	pub fn abs(&self) -> &Path {
		match self {
			Self::Local(_, abs) | Self::Remote(_, abs) => abs,
		}
	}

	pub fn root(&self) -> &Path {
		match self {
			Self::Local(source, _) => &source.root,
			Self::Remote(share, _) => &share.info.root,
		}
	}

	pub fn byte_target(&self) -> ByteTarget {
		let source_id = match self {
			Self::Local(source, _) => source.id,
			Self::Remote(share, _) => share.info.id,
		};
		ByteTarget {
			source_id,
			path: self.abs().to_path_buf(),
		}
	}
}

/// Stable share directory name for a source: root basename + short id, so
/// two drives named "Backup" stay distinguishable and renames of the mount
/// point don't orphan client bookmarks within a session.
pub fn share_name(root: &Path, id: Uuid) -> String {
	let base = root
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| {
			// URI roots ("s3://bucket") and bare "/" have no file_name.
			let s = root.to_string_lossy();
			let trimmed = s
				.trim_end_matches('/')
				.rsplit(['/', ':'])
				.next()
				.unwrap_or("")
				.to_string();
			if trimmed.is_empty() {
				"root".to_string()
			} else {
				trimmed
			}
		});
	let base: String = base
		.chars()
		.map(|c| if c == '/' || c.is_control() { '_' } else { c })
		.collect();
	// Source ids are uuid v7: the leading characters are the mint timestamp
	// and collide for ids created in the same instant, so the discriminating
	// suffix comes from the random tail.
	let id = id.simple().to_string();
	format!("{}-{}", base, &id[id.len() - 6..])
}

/// Share directory name for a replicated peer source: basename @ device.
pub fn remote_share_name(share: &super::peer::RemoteShare) -> String {
	let base = share_name(&share.info.root, share.info.id);
	let device: String = share
		.device_label
		.chars()
		.map(|c| if c == '/' || c.is_control() { '_' } else { c })
		.collect();
	// share_name ends with the uuid tail; splice the device label before it.
	match base.rfind('-') {
		Some(pos) => format!("{}@{}-{}", &base[..pos], device, &base[pos + 1..]),
		None => format!("{base}@{device}"),
	}
}

pub async fn resolve_target(context: &Arc<CoreContext>, rel: &str) -> Option<MountTarget> {
	let mut parts = rel.splitn(2, '/');
	let share = parts.next()?;
	let subpath = parts.next().unwrap_or("");

	let sub = Path::new(subpath);
	if sub.components().any(|c| !matches!(c, Component::Normal(_))) {
		return None;
	}

	if let Some((source, abs)) = resolve(&context.volume_index().sources(), rel) {
		return Some(MountTarget::Local(source, abs));
	}

	for remote in super::peer::remote_shares().await {
		if remote_share_name(&remote) == share {
			let abs = if subpath.is_empty() {
				remote.info.root.clone()
			} else {
				remote.info.root.join(sub)
			};
			return Some(MountTarget::Remote(remote, abs));
		}
	}
	None
}

/// Resolve a decoded share-relative path ("Share-abc123/some/dir") to the
/// source it belongs to and the absolute path inside that source.
#[cfg(test)]
pub fn resolve_for_test(sources: &[SourceStatus], rel: &str) -> Option<(SourceStatus, PathBuf)> {
	resolve(sources, rel)
}

fn resolve(sources: &[SourceStatus], rel: &str) -> Option<(SourceStatus, PathBuf)> {
	let mut parts = rel.splitn(2, '/');
	let share = parts.next()?;
	let subpath = parts.next().unwrap_or("");

	let source = sources
		.iter()
		.find(|s| share_name(&s.root, s.id) == share)?
		.clone();

	// Refuse traversal: only plain names may join onto the source root.
	let sub = Path::new(subpath);
	if sub.components().any(|c| !matches!(c, Component::Normal(_))) {
		return None;
	}
	let abs = if subpath.is_empty() {
		source.root.clone()
	} else {
		source.root.join(sub)
	};
	Some((source, abs))
}

/// The provider that can serve a target's bytes, or the reason none can.
///
/// Peer and cloud providers are wrapped in the block cache; local ones are
/// not, because their bytes are already on a local disk and a second copy of
/// them is waste.
pub async fn provider_for(
	context: &Arc<CoreContext>,
	target: &MountTarget,
) -> Result<Arc<dyn ByteProvider>, ByteError> {
	match target {
		MountTarget::Remote(share, _) => Ok(cached(Arc::new(PeerProvider {
			context: context.clone(),
			device_id: share.device_id,
			device_label: share.device_label.clone(),
		}))),
		MountTarget::Local(source, abs) => {
			if is_uri_root(&source.root) {
				return Ok(cached(cloud_provider(context, source, abs).await?));
			}
			if !source.attached || context.volume_index().is_detached(abs) {
				return Err(ByteError::Detached {
					source: source.root.clone(),
				});
			}
			Ok(Arc::new(LocalProvider::open(abs).await?))
		}
	}
}

/// Put the block cache in front of a provider whose bytes come off the
/// network. Without a cache (a daemon that never started the share) the
/// provider is served straight through.
fn cached(inner: Arc<dyn ByteProvider>) -> Arc<dyn ByteProvider> {
	match super::cache::cache() {
		Some(cache) => Arc::new(super::cache::CachedProvider::new(inner, cache)),
		None => inner,
	}
}

pub fn is_uri_root(root: &Path) -> bool {
	root.to_string_lossy().contains("://")
}

async fn cloud_provider(
	context: &Arc<CoreContext>,
	source: &SourceStatus,
	abs: &Path,
) -> Result<Arc<dyn ByteProvider>, ByteError> {
	let Some(volume_uuid) = source.volume_uuid else {
		return Err(ByteError::Unsupported("source has no volume".into()));
	};
	let Some(mut volume) = context
		.volume_manager
		.get_all_volumes()
		.await
		.into_iter()
		.find(|volume| volume.id == volume_uuid)
	else {
		return Err(ByteError::NoBackend("cloud volume not registered".into()));
	};
	if volume.backend.is_none() {
		// backend_for_volume would fabricate a LocalBackend over the URI
		// string; refuse instead of reading a literal "s3://…" path.
		return Err(ByteError::NoBackend(
			"cloud volume has no backend (credentials not restored)".into(),
		));
	}
	let backend = context.volume_manager.backend_for_volume(&mut volume);

	let abs_str = abs.to_string_lossy();
	let root_str = source.root.to_string_lossy();
	let key = abs_str
		.strip_prefix(root_str.as_ref())
		.unwrap_or(&abs_str)
		.trim_start_matches('/')
		.to_string();

	Ok(Arc::new(CloudProvider {
		backend,
		key: PathBuf::from(key),
	}))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn target() -> ByteTarget {
		ByteTarget {
			source_id: Uuid::nil(),
			path: PathBuf::new(),
		}
	}

	async fn fixture(bytes: &[u8]) -> (tempfile::TempDir, LocalProvider) {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("clip.bin");
		std::fs::write(&path, bytes).unwrap();
		let provider = LocalProvider::open(&path).await.unwrap();
		(dir, provider)
	}

	#[tokio::test]
	async fn local_ranges_are_byte_exact() {
		let data: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
		let (_dir, provider) = fixture(&data).await;
		let t = target();

		assert_eq!(provider.stat(&t).await.unwrap().size, 4096);

		let whole = provider.read_range(&t, 0..4096).await.unwrap();
		assert_eq!(&whole[..], &data[..]);

		let middle = provider.read_range(&t, 1000..1500).await.unwrap();
		assert_eq!(&middle[..], &data[1000..1500]);

		let tail = provider.read_range(&t, 4000..4096).await.unwrap();
		assert_eq!(&tail[..], &data[4000..]);
	}

	#[tokio::test]
	async fn local_reads_are_position_independent() {
		// Positional reads share one handle with no seek cursor, so ranges
		// issued concurrently and out of order must not disturb each other.
		let data: Vec<u8> = (0..8192u32).map(|i| (i % 253) as u8).collect();
		let (_dir, provider) = fixture(&data).await;
		let provider = Arc::new(provider);

		let mut handles = Vec::new();
		for start in [7000u64, 0, 4096, 512] {
			let provider = provider.clone();
			handles.push(tokio::spawn(async move {
				let got = provider
					.read_range(&target(), start..start + 512)
					.await
					.unwrap();
				(start, got)
			}));
		}
		for handle in handles {
			let (start, got) = handle.await.unwrap();
			let s = start as usize;
			assert_eq!(&got[..], &data[s..s + 512], "range at {start}");
		}
	}

	#[tokio::test]
	async fn local_short_reads_and_edges() {
		let (_dir, provider) = fixture(b"0123456789").await;
		let t = target();

		// Asking past EOF comes back short rather than erroring; the caller
		// loop treats a short read as the end.
		let past = provider.read_range(&t, 8..64).await.unwrap();
		assert_eq!(&past[..], b"89");

		// Entirely past EOF is empty, which stops the streaming loop.
		assert!(provider.read_range(&t, 32..64).await.unwrap().is_empty());

		// Zero-length never touches the file.
		assert!(provider.read_range(&t, 4..4).await.unwrap().is_empty());
	}

	#[tokio::test]
	async fn missing_file_is_not_found() {
		let dir = tempfile::tempdir().unwrap();
		let err = LocalProvider::open(&dir.path().join("absent")).await;
		assert!(matches!(err, Err(ByteError::NotFound(_))));
	}

	#[test]
	fn error_messages_match_what_clients_saw() {
		assert_eq!(
			ByteError::Detached {
				source: PathBuf::from("/Volumes/Archive")
			}
			.to_string(),
			"source detached: /Volumes/Archive"
		);
		assert_eq!(
			ByteError::PeerUnavailable {
				device: "studio-mac".into(),
				reason: "device is not connected".into(),
			}
			.to_string(),
			"peer studio-mac unavailable: device is not connected"
		);
	}

	#[test]
	fn uri_roots_route_to_the_backend() {
		assert!(is_uri_root(Path::new("s3://bucket")));
		assert!(!is_uri_root(Path::new("/Volumes/Footage")));
	}
}
