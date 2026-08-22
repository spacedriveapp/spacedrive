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

fn dir_info(name: String, modified: Option<SystemTime>) -> FileInfo {
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
		file_index: 0,
	}
}

fn file_info(name: String, size: u64, modified: Option<SystemTime>) -> FileInfo {
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
		file_index: 0,
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
			.ephemeral_cache()
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
				let cache = self.context.ephemeral_cache();
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

		if is_directory {
			return Ok(Box::new(DirHandle {
				index,
				abs,
				info: dir_info(name, meta.as_ref().and_then(|m| m.modified)),
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
		Ok(dir_info(SHARE.to_string(), None))
	}

	async fn set_times(&self, _times: FileTimes) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn truncate(&self, _len: u64) -> SmbResult<()> {
		Err(SmbError::AccessDenied)
	}

	async fn list_dir(&self, _pattern: Option<&str>) -> SmbResult<Vec<DirEntry>> {
		Ok(self
			.share
			.source_names()
			.await
			.into_iter()
			.map(|(name, modified)| DirEntry {
				info: dir_info(name, modified),
			})
			.collect())
	}

	async fn close(self: Box<Self>) -> SmbResult<()> {
		Ok(())
	}
}

struct DirHandle {
	index: Arc<tokio::sync::RwLock<crate::ops::indexing::ephemeral::index::EphemeralIndex>>,
	abs: PathBuf,
	info: FileInfo,
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

	async fn list_dir(&self, _pattern: Option<&str>) -> SmbResult<Vec<DirEntry>> {
		let index = self.index.read().await;
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
			out.push(DirEntry {
				info: if meta.kind == EntryKind::Directory {
					dir_info(name, meta.modified)
				} else {
					file_info(name, meta.size, meta.modified)
				},
			});
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
			info: file_info("a001.braw".into(), size, None),
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
		let dir = dir_info("Footage".into(), None);
		assert!(dir.is_directory);
		assert_eq!(dir.end_of_file, 0);
		assert_eq!(dir.attributes(), 0x10);

		let file = file_info("a001.braw".into(), 24_300_000_000, None);
		assert!(!file.is_directory);
		assert_eq!(file.end_of_file, 24_300_000_000);
		// Allocation size matching end-of-file is what makes a streamed file
		// report its real size rather than its resident size.
		assert_eq!(file.allocation_size, file.end_of_file);
		assert_eq!(file.attributes(), 0x80);
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
