//! Loopback SMB frontend.
//!
//! An SMB2 server bound to 127.0.0.1 that the host's own SMB client mounts.
//! No driver, no system extension, no elevation — which makes it the mount
//! that works on a machine where nothing can be installed, and the honest
//! baseline a native module has to beat.
//!
//! Metadata comes from the same arenas the WebDAV share and the Explorer
//! read; bytes come from the byte plane, so peer and cloud sources are
//! cached and read ahead exactly as they are everywhere else. The share is
//! read-only: `BackendCapabilities::is_read_only` makes the protocol layer
//! refuse every write before it reaches us.

use super::provider::{self, remote_share_name, share_name, ByteProvider, ByteTarget, MountTarget};
use crate::context::CoreContext;
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::EntryKind;
use async_trait::async_trait;
use smb_server::{
	Access, BackendCapabilities, DirEntry, FileInfo, FileTimes, Handle, OpenOptions, Share,
	ShareBackend, SmbError, SmbPath, SmbResult, SmbServer,
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

/// Share name the whole library is exposed under, so one mount shows every
/// source rather than the user mounting each drive separately. Per-source
/// shares stay open as a question in the mounts design.
const SHARE: &str = "spacedrive";

/// Windows epoch offset in seconds: 1601-01-01 to 1970-01-01.
const FILETIME_EPOCH_OFFSET: u64 = 11_644_473_600;

/// The single account the share is served to. Real SMB clients do not send
/// a true NTLM anonymous logon — macOS in particular authenticates even for
/// a guest mount — so the share carries one credential rather than relying
/// on an anonymous bind that clients will not use.
const USER: &str = "spacedrive";

static BOUND_ADDR: OnceLock<SocketAddr> = OnceLock::new();
static PASSWORD: OnceLock<String> = OnceLock::new();

/// Address the SMB server is listening on, when running.
pub fn bound_addr() -> Option<SocketAddr> {
	BOUND_ADDR.get().copied()
}

/// The URL a host SMB client mounts, credentials included. The server is
/// bound to loopback and the password is regenerated every start, so this
/// is a handle for this process's lifetime rather than a secret.
pub fn mount_url() -> Option<String> {
	let addr = bound_addr()?;
	let password = PASSWORD.get()?;
	Some(format!(
		"smb://{USER}:{password}@{}:{}/{SHARE}",
		addr.ip(),
		addr.port()
	))
}

/// How this process mounts its own share: where the server listens and the
/// login it accepts.
pub(super) struct Login {
	/// `smb://127.0.0.1:<port>/spacedrive`, without credentials.
	pub url: String,
	/// The mounted share as the mount table names it, which tells this run's
	/// mount apart from one an earlier run left behind.
	pub source: String,
	pub user: &'static str,
	pub password: &'static str,
}

/// The login for this run's server, when it is running.
pub(super) fn login() -> Option<Login> {
	let addr = bound_addr()?;
	let password = PASSWORD.get()?;
	Some(Login {
		url: format!("smb://{}:{}/{SHARE}", addr.ip(), addr.port()),
		source: format!("//{USER}@{}:{}/{SHARE}", addr.ip(), addr.port()),
		user: USER,
		password,
	})
}

/// Password for this run: the environment when set (so a long-lived mount
/// survives a daemon restart), otherwise freshly random.
fn password() -> String {
	if let Ok(from_env) = std::env::var("SD_MOUNT_SMB_PASSWORD") {
		if !from_env.is_empty() {
			return from_env;
		}
	}
	let bytes: [u8; 16] = rand::random();
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn to_filetime(time: Option<SystemTime>) -> u64 {
	let Some(time) = time else { return 0 };
	let Ok(since) = time.duration_since(SystemTime::UNIX_EPOCH) else {
		return 0;
	};
	(since.as_secs() + FILETIME_EPOCH_OFFSET) * 10_000_000 + (since.subsec_nanos() as u64) / 100
}

/// The file id the share reports for a share-relative path.
///
/// The macOS client keys the nodes it builds from a directory listing on this
/// id, so two entries sharing one would read as a single file. Hashing the
/// path gives every entry its own id, and the same one whether the client
/// learns it from a listing or from opening the path. Zero means "no id" to
/// clients.
fn file_id(rel: &str) -> u64 {
	let hash = blake3::hash(rel.as_bytes());
	let mut id = [0u8; 8];
	id.copy_from_slice(&hash.as_bytes()[..8]);
	u64::from_le_bytes(id).max(1)
}

/// Whether a directory entry named `name` answers a client's search
/// `pattern`.
///
/// A client lists a directory with `*`, which reaches the backend as no
/// pattern at all, and looks one name up by searching for that name alone and
/// taking the first entry back as its answer. So an entry the pattern does not
/// name must never be returned. `*` matches any run of characters and `?` any
/// one; the DOS forms `<`, `>` and `"` are read as `*`, `?` and `.`. Case is
/// ignored, as the share declares itself case-insensitive.
fn name_matches(pattern: &str, name: &str) -> bool {
	let pattern: Vec<char> = pattern
		.chars()
		.flat_map(char::to_lowercase)
		.map(|c| match c {
			'<' => '*',
			'>' => '?',
			'"' => '.',
			c => c,
		})
		.collect();
	let name: Vec<char> = name.chars().flat_map(char::to_lowercase).collect();

	let (mut p, mut n) = (0, 0);
	// The last `*` seen, and where in the name it started matching, so a
	// mismatch after it retries with the star taking one more character.
	let mut star: Option<(usize, usize)> = None;
	while n < name.len() {
		match pattern.get(p) {
			Some('?') => {
				p += 1;
				n += 1;
			}
			Some('*') => {
				star = Some((p, n));
				p += 1;
			}
			Some(c) if *c == name[n] => {
				p += 1;
				n += 1;
			}
			_ => match star {
				Some((star_at, from)) => {
					p = star_at + 1;
					n = from + 1;
					star = Some((star_at, from + 1));
				}
				None => return false,
			},
		}
	}
	pattern[p..].iter().all(|c| *c == '*')
}

/// Whether a search pattern names one entry rather than matching several.
fn is_exact(pattern: &str) -> bool {
	!pattern.contains(['*', '?', '<', '>', '"'])
		&& Path::new(pattern)
			.components()
			.map(|component| matches!(component, std::path::Component::Normal(_)))
			.eq([true])
}

/// The share-relative path of `name` inside the directory at `parent`.
fn child_rel(parent: &str, name: &str) -> String {
	if parent.is_empty() {
		name.to_string()
	} else {
		format!("{parent}/{name}")
	}
}

fn dir_info(name: String, id: u64, modified: Option<SystemTime>) -> FileInfo {
	let ft = to_filetime(modified);
	FileInfo {
		name,
		end_of_file: 0,
		allocation_size: 0,
		creation_time: ft,
		last_access_time: ft,
		last_write_time: ft,
		change_time: ft,
		is_directory: true,
		file_index: id,
	}
}

fn file_info(name: String, id: u64, size: u64, modified: Option<SystemTime>) -> FileInfo {
	let ft = to_filetime(modified);
	FileInfo {
		name,
		end_of_file: size,
		allocation_size: size,
		creation_time: ft,
		last_access_time: ft,
		last_write_time: ft,
		change_time: ft,
		is_directory: false,
		file_index: id,
	}
}

/// Every registered source, exposed as a top-level directory.
pub struct LibraryShare {
	context: Arc<CoreContext>,
}

impl LibraryShare {
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self { context }
	}

	/// Names of the top-level directories: one per local source, one per
	/// replicated peer source.
	async fn source_names(&self) -> Vec<(String, Option<SystemTime>)> {
		let mut names: Vec<(String, Option<SystemTime>)> = self
			.context
			.volume_index()
			.sources()
			.into_iter()
			.map(|source| (share_name(&source.root, source.id), None))
			.collect();
		for remote in super::peer::remote_shares().await {
			names.push((remote_share_name(&remote), None));
		}
		names
	}
}

#[async_trait]
impl ShareBackend for LibraryShare {
	async fn open(&self, path: &SmbPath, opts: OpenOptions) -> SmbResult<Box<dyn Handle>> {
		if opts.write || opts.delete_on_close {
			return Err(SmbError::AccessDenied);
		}

		let rel = path.components().join("/");

		// The share root itself: a directory listing every source.
		if path.is_root() || rel.is_empty() {
			if opts.non_directory {
				return Err(SmbError::IsDirectory);
			}
			return Ok(Box::new(RootHandle {
				share: LibraryShare {
					context: self.context.clone(),
				},
			}));
		}

		let Some(target) = provider::resolve_target(&self.context, &rel).await else {
			return Err(SmbError::NotFound);
		};

		let abs = target.abs().to_path_buf();
		let root = target.root().to_path_buf();
		let is_source_root = abs == root;

		let (index, meta) = match &target {
			MountTarget::Local(source, _) => {
				let cache = self.context.volume_index();
				cache.ensure_restored(&source.root).await;
				let index = cache.resolve_index(&abs);
				let meta = index.read().await.get_entry_ref(&abs);
				(index, meta)
			}
			MountTarget::Remote(share, _) => {
				let index = share.index.clone();
				let meta = index.read().await.get_entry_ref(&abs);
				(index, meta)
			}
		};

		let is_directory = is_source_root
			|| meta
				.as_ref()
				.map(|m| m.kind == EntryKind::Directory)
				.unwrap_or(false);

		if !is_source_root && meta.is_none() {
			return Err(SmbError::NotFound);
		}
		if is_directory && opts.non_directory {
			return Err(SmbError::IsDirectory);
		}
		if !is_directory && opts.directory {
			return Err(SmbError::NotADirectory);
		}

		let name = display_name(&abs, &rel);
		let id = file_id(&rel);

		if is_directory {
			return Ok(Box::new(DirHandle {
				index,
				abs,
				info: dir_info(name, id, meta.as_ref().and_then(|m| m.modified)),
				rel,
			}));
		}

		// A file: resolve its byte provider now, so a CREATE that cannot be
		// served fails here rather than at first read.
		let provider = provider::provider_for(&self.context, &target)
			.await
			.map_err(byte_error)?;
		let byte_target = target.byte_target();
		let stat = provider.stat(&byte_target).await.map_err(byte_error)?;

		Ok(Box::new(FileHandle {
			trace_id: super::trace::file_id(&byte_target.path),
			provider,
			target: byte_target,
			info: file_info(
				name,
				id,
				stat.size,
				stat.modified.or(meta.and_then(|m| m.modified)),
			),
		}))
	}

	async fn unlink(&self, _path: &SmbPath) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn rename(&self, _from: &SmbPath, _to: &SmbPath) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	fn capabilities(&self) -> BackendCapabilities {
		BackendCapabilities {
			is_read_only: true,
			// macOS and Windows both mount case-insensitively; claiming
			// sensitivity here makes Finder miss files it just listed.
			case_sensitive: false,
		}
	}
}

fn display_name(abs: &Path, rel: &str) -> String {
	abs.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| rel.rsplit('/').next().unwrap_or(SHARE).to_string())
}

/// A detached drive or an unreachable peer is not an error the SMB client
/// can render usefully, so reads map to "device not ready" — the closest
/// thing the protocol has to "this drive is in a drawer".
fn byte_error(err: provider::ByteError) -> SmbError {
	match err {
		provider::ByteError::NotFound(_) => SmbError::NotFound,
		_ => {
			tracing::debug!("smb: byte plane unavailable: {err}");
			SmbError::Io(std::io::Error::other(err.to_string()))
		}
	}
}

// ----------------------------------------------------------------- handles

struct RootHandle {
	share: LibraryShare,
}

#[async_trait]
impl Handle for RootHandle {
	async fn read(&self, _offset: u64, _len: u32) -> SmbResult<bytes::Bytes> {
		Err(SmbError::IsDirectory)
	}

	async fn write(&self, _offset: u64, _data: &[u8]) -> SmbResult<u32> {
		Err(SmbError::AccessDenied)
	}

	async fn flush(&self) -> SmbResult<()> {
		Ok(())
	}

	async fn stat(&self) -> SmbResult<FileInfo> {
		Ok(dir_info(SHARE.to_string(), file_id(""), None))
	}

	async fn set_times(&self, _times: FileTimes) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn truncate(&self, _len: u64) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn list_dir(&self, pattern: Option<&str>) -> SmbResult<Vec<DirEntry>> {
		Ok(self
			.share
			.source_names()
			.await
			.into_iter()
			.filter(|(name, _)| pattern.is_none_or(|pattern| name_matches(pattern, name)))
			.map(|(name, modified)| DirEntry {
				info: dir_info(name.clone(), file_id(&name), modified),
			})
			.collect())
	}

	async fn close(self: Box<Self>) -> SmbResult<()> {
		Ok(())
	}
}

struct DirHandle {
	index: Arc<tokio::sync::RwLock<crate::ops::indexing::arena::Arena>>,
	abs: PathBuf,
	info: FileInfo,
	/// The directory's share-relative path, which its entries' ids hash under.
	rel: String,
}

impl DirHandle {
	/// The listing entry for the child called `name`.
	fn entry(&self, name: String, meta: &EntryMetadata) -> DirEntry {
		let id = file_id(&child_rel(&self.rel, &name));
		DirEntry {
			info: if meta.kind == EntryKind::Directory {
				dir_info(name, id, meta.modified)
			} else {
				file_info(name, id, meta.size, meta.modified)
			},
		}
	}
}

#[async_trait]
impl Handle for DirHandle {
	async fn read(&self, _offset: u64, _len: u32) -> SmbResult<bytes::Bytes> {
		Err(SmbError::IsDirectory)
	}

	async fn write(&self, _offset: u64, _data: &[u8]) -> SmbResult<u32> {
		Err(SmbError::AccessDenied)
	}

	async fn flush(&self) -> SmbResult<()> {
		Ok(())
	}

	async fn stat(&self) -> SmbResult<FileInfo> {
		Ok(self.info.clone())
	}

	async fn set_times(&self, _times: FileTimes) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn truncate(&self, _len: u64) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn list_dir(&self, pattern: Option<&str>) -> SmbResult<Vec<DirEntry>> {
		let index = self.index.read().await;
		// A lookup of one name reads that entry rather than the directory.
		if let Some(name) = pattern.filter(|pattern| is_exact(pattern)) {
			if let Some(meta) = index.get_entry_ref(&self.abs.join(name)) {
				return Ok(vec![self.entry(name.to_string(), &meta)]);
			}
		}
		let Some(children) = index.list_directory(&self.abs) else {
			return Ok(Vec::new());
		};
		let mut out = Vec::with_capacity(children.len());
		for child in children {
			let Some(meta) = index.get_entry_ref(&child) else {
				continue;
			};
			let name = child
				.file_name()
				.map(|n| n.to_string_lossy().into_owned())
				.unwrap_or_default();
			if pattern.is_some_and(|pattern| !name_matches(pattern, &name)) {
				continue;
			}
			out.push(self.entry(name, &meta));
		}
		Ok(out)
	}

	async fn close(self: Box<Self>) -> SmbResult<()> {
		Ok(())
	}
}

struct FileHandle {
	provider: Arc<dyn ByteProvider>,
	target: ByteTarget,
	info: FileInfo,
	/// Stable id for the read trace, so a session's reads group by file.
	trace_id: u64,
}

#[async_trait]
impl Handle for FileHandle {
	async fn read(&self, offset: u64, len: u32) -> SmbResult<bytes::Bytes> {
		if offset >= self.info.end_of_file {
			return Ok(bytes::Bytes::new());
		}
		let end = (offset + len as u64).min(self.info.end_of_file);
		let started = std::time::Instant::now();
		let bytes = self
			.provider
			.read_range(&self.target, offset..end)
			.await
			.map_err(byte_error)?;
		super::trace::record(
			super::trace::Frontend::Smb,
			self.trace_id,
			offset,
			bytes.len() as u64,
			started,
		);
		Ok(bytes)
	}

	async fn write(&self, _offset: u64, _data: &[u8]) -> SmbResult<u32> {
		Err(SmbError::AccessDenied)
	}

	async fn flush(&self) -> SmbResult<()> {
		Ok(())
	}

	async fn stat(&self) -> SmbResult<FileInfo> {
		Ok(self.info.clone())
	}

	async fn set_times(&self, _times: FileTimes) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn truncate(&self, _len: u64) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn list_dir(&self, _pattern: Option<&str>) -> SmbResult<Vec<DirEntry>> {
		Err(SmbError::NotADirectory)
	}

	async fn close(self: Box<Self>) -> SmbResult<()> {
		Ok(())
	}
}

// ------------------------------------------------------------------ server

/// Bind the SMB server on loopback and serve it for the life of the
/// process. Port 0 by default so the OS picks a free high port — 445 is
/// privileged and usually already taken by the host's own SMB service.
pub async fn start(context: Arc<CoreContext>) -> anyhow::Result<SocketAddr> {
	let port: u16 = std::env::var("SD_MOUNT_SMB_PORT")
		.ok()
		.and_then(|p| p.parse().ok())
		.unwrap_or(0);

	let password = password();
	let server = SmbServer::builder()
		.listen(SocketAddr::from(([127, 0, 0, 1], port)))
		.netbios_name("SPACEDRIVE")
		// One read per block run in the byte plane; matching the provider
		// ceiling keeps a client's large read from being split twice.
		.max_read_size(4 * 1024 * 1024)
		.user(USER, password.clone())
		.share(Share::new(SHARE, LibraryShare::new(context)).user(USER, Access::Read))
		.build()?;
	let _ = PASSWORD.set(password);

	let addr = server.bind().await?;
	let _ = BOUND_ADDR.set(addr);

	tokio::spawn(async move {
		if let Err(err) = server.serve().await {
			tracing::error!("Mounts SMB server exited: {err}");
		}
	});

	// The password is in the URL, so it goes to the log the operator already
	// has to read to find the port. Loopback-only and per-process.
	tracing::info!("Mounts SMB share at {}", mount_url().unwrap_or_default());
	Ok(addr)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::mounts::provider::{ByteError, ByteStat, ProviderClass};
	use std::ops::Range;
	use std::time::Duration;

	struct StaticProvider {
		data: bytes::Bytes,
	}

	#[async_trait]
	impl ByteProvider for StaticProvider {
		async fn stat(&self, _t: &ByteTarget) -> Result<ByteStat, ByteError> {
			Ok(ByteStat {
				size: self.data.len() as u64,
				modified: None,
			})
		}

		async fn read_range(
			&self,
			_t: &ByteTarget,
			range: Range<u64>,
		) -> Result<bytes::Bytes, ByteError> {
			let start = (range.start as usize).min(self.data.len());
			let end = (range.end as usize).min(self.data.len());
			Ok(self.data.slice(start..end))
		}

		fn class(&self) -> ProviderClass {
			ProviderClass::Peer
		}

		fn max_read(&self) -> u64 {
			4 * 1024 * 1024
		}
	}

	fn file_handle(data: &[u8]) -> FileHandle {
		let data = bytes::Bytes::copy_from_slice(data);
		let size = data.len() as u64;
		FileHandle {
			trace_id: 0,
			provider: Arc::new(StaticProvider { data }),
			target: ByteTarget {
				source_id: uuid::Uuid::nil(),
				path: PathBuf::from("/src/a001.braw"),
			},
			info: file_info("a001.braw".into(), file_id("src/a001.braw"), size, None),
		}
	}

	#[tokio::test]
	async fn reads_are_byte_exact_and_clamped() {
		let data: Vec<u8> = (0..2048u32).map(|i| (i % 251) as u8).collect();
		let handle = file_handle(&data);

		let head = handle.read(0, 512).await.unwrap();
		assert_eq!(&head[..], &data[0..512]);

		let middle = handle.read(1000, 24).await.unwrap();
		assert_eq!(&middle[..], &data[1000..1024]);

		// A client asking past the end gets what exists, not an error — SMB
		// clients routinely request a full buffer at the tail of a file.
		let tail = handle.read(2000, 4096).await.unwrap();
		assert_eq!(&tail[..], &data[2000..]);

		// Entirely past the end is empty, which is how the client learns EOF.
		assert!(handle.read(4096, 512).await.unwrap().is_empty());
	}

	#[tokio::test]
	async fn the_share_refuses_every_write() {
		let handle = file_handle(b"read only");
		assert!(matches!(
			handle.write(0, b"nope").await,
			Err(SmbError::AccessDenied)
		));
		assert!(matches!(
			handle.truncate(0).await,
			Err(SmbError::AccessDenied)
		));
		assert!(matches!(
			handle.set_times(FileTimes::default()).await,
			Err(SmbError::AccessDenied)
		));
		// A file is not a directory, and saying so is what stops a client
		// walking into it.
		assert!(matches!(
			handle.list_dir(None).await,
			Err(SmbError::NotADirectory)
		));
	}

	#[test]
	fn filetime_uses_the_windows_epoch() {
		// The Unix epoch is 11644473600 seconds after 1601-01-01, in 100ns
		// ticks. Getting this wrong dates every file in Finder to 1601.
		assert_eq!(
			to_filetime(Some(SystemTime::UNIX_EPOCH)),
			11_644_473_600 * 10_000_000
		);
		let later = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
		assert_eq!(
			to_filetime(Some(later)),
			(11_644_473_600 + 1_000_000_000) * 10_000_000
		);
		// Sub-second precision survives at 100ns granularity.
		let precise = SystemTime::UNIX_EPOCH + Duration::from_nanos(1_500_000_000);
		assert_eq!(
			to_filetime(Some(precise)),
			(11_644_473_600 + 1) * 10_000_000 + 5_000_000
		);
		// No timestamp is zero, which clients render as "unknown" rather
		// than as the Windows epoch.
		assert_eq!(to_filetime(None), 0);
	}

	#[test]
	fn directories_and_files_carry_the_right_attributes() {
		let dir = dir_info("Footage".into(), file_id("Footage"), None);
		assert!(dir.is_directory);
		assert_eq!(dir.end_of_file, 0);
		assert_eq!(dir.attributes(), 0x10);

		let file = file_info(
			"a001.braw".into(),
			file_id("Footage/a001.braw"),
			24_300_000_000,
			None,
		);
		assert!(!file.is_directory);
		assert_eq!(file.end_of_file, 24_300_000_000);
		// Allocation size matching end-of-file is what makes a streamed file
		// report its real size rather than its resident size.
		assert_eq!(file.allocation_size, file.end_of_file);
		assert_eq!(file.attributes(), 0x80);
	}

	#[test]
	fn search_patterns_match_as_the_protocol_asks() {
		assert!(name_matches("clip.mp4", "clip.mp4"));
		assert!(name_matches("CLIP.MP4", "clip.mp4"));
		assert!(!name_matches("clip.mp4", "tail-moov.mov"));
		assert!(!name_matches("clip", "clip.mp4"));
		assert!(name_matches("*", "clip.mp4"));
		assert!(name_matches("*.mov", "tail-moov.MOV"));
		assert!(!name_matches("*.mov", "clip.mp4"));
		assert!(name_matches("c?ip.*", "clip.mp4"));
		assert!(name_matches("*moov*", "tail-moov.mov"));
		assert!(name_matches("<.mov", "tail-moov.mov"));
		assert!(is_exact("clip.mp4"));
		assert!(!is_exact("*.mov"));
		assert!(!is_exact("../clip.mp4"));
		assert!(!is_exact("a/clip.mp4"));
		assert!(!is_exact(""));
	}

	fn listed_names(entries: &[DirEntry]) -> Vec<&str> {
		entries
			.iter()
			.map(|entry| entry.info.name.as_str())
			.collect()
	}

	/// A lookup of one name answers with that entry alone. macOS takes the
	/// first entry back as the name's, so answering with the directory's first
	/// entry would give one file another's id, size and bytes.
	#[tokio::test]
	async fn a_name_lookup_answers_with_that_entry_alone() {
		let root = PathBuf::from("/src/clips");
		let mut arena = crate::ops::indexing::arena::Arena::new().expect("arena");
		for (name, size) in [("tail-moov.mov", 10), ("clip.mp4", 60)] {
			let path = root.join(name);
			let metadata = EntryMetadata {
				path: path.clone(),
				kind: EntryKind::File,
				size,
				modified: None,
				accessed: None,
				created: None,
				inode: None,
				permissions: None,
				uid: None,
				gid: None,
				link_target: None,
				is_hidden: false,
			};
			arena
				.add_entry(path, uuid::Uuid::now_v7(), metadata)
				.expect("entry");
		}
		let dir = DirHandle {
			index: Arc::new(tokio::sync::RwLock::new(arena)),
			abs: root,
			info: dir_info("clips".into(), file_id("src/clips"), None),
			rel: "src/clips".into(),
		};

		let found = dir.list_dir(Some("clip.mp4")).await.unwrap();
		assert_eq!(listed_names(&found), ["clip.mp4"]);
		assert_eq!(found[0].info.end_of_file, 60);
		assert_eq!(found[0].info.file_index, file_id("src/clips/clip.mp4"));

		assert_eq!(
			listed_names(&dir.list_dir(Some("CLIP.MP4")).await.unwrap()),
			["clip.mp4"]
		);
		assert!(dir.list_dir(Some("._clip.mp4")).await.unwrap().is_empty());
		assert_eq!(
			listed_names(&dir.list_dir(Some("*.mov")).await.unwrap()),
			["tail-moov.mov"]
		);

		let all = dir.list_dir(None).await.unwrap();
		assert_eq!(all.len(), 2);
		assert_ne!(all[0].info.file_index, all[1].info.file_index);
	}

	/// Every entry gets its own id, the same one a listing and an open of its
	/// path report, and never the zero clients read as "no id".
	#[test]
	fn file_ids_follow_the_path() {
		let listed = file_id(&child_rel("Footage-a1b2c3/day1", "a001.braw"));
		let opened = file_id("Footage-a1b2c3/day1/a001.braw");
		assert_eq!(listed, opened);
		assert_ne!(opened, file_id("Footage-a1b2c3/day1/a002.braw"));
		assert_ne!(file_id(""), file_id("Footage-a1b2c3"));
		assert_eq!(child_rel("", "Footage-a1b2c3"), "Footage-a1b2c3");
		for rel in ["", "Footage-a1b2c3", "Footage-a1b2c3/day1/a001.braw"] {
			assert_ne!(file_id(rel), 0);
		}
	}

	#[test]
	fn detached_and_unreachable_map_to_retryable_errors() {
		// A drive in a drawer is not "not found" — telling a client the file
		// is gone invites it to forget the file.
		assert!(matches!(
			byte_error(ByteError::Detached {
				source: PathBuf::from("/Volumes/Archive")
			}),
			SmbError::Io(_)
		));
		assert!(matches!(
			byte_error(ByteError::PeerUnavailable {
				device: "studio".into(),
				reason: "offline".into()
			}),
			SmbError::Io(_)
		));
		assert!(matches!(
			byte_error(ByteError::NotFound(String::new())),
			SmbError::NotFound
		));
	}
}
