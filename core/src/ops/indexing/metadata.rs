//! Filesystem metadata as the indexer reads it.
//!
//! A stat is paid for once, during the walk that visited the entry, and
//! [`EntryMetadata`] is what that stat produced. Everything downstream reads
//! this rather than touching the filesystem a second time.

use std::path::{Path, PathBuf};

use super::state::{DirEntry, EntryKind};

/// Check if a filesystem path should be treated as hidden.
///
/// On Windows, uses `FILE_ATTRIBUTE_HIDDEN` via `GetFileAttributesW` exclusively.
/// On other platforms, uses the dot-prefix convention.
pub fn is_hidden_path(path: &Path) -> bool {
	#[cfg(windows)]
	{
		use std::os::windows::ffi::OsStrExt;
		use windows_sys::Win32::Storage::FileSystem::{
			GetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, INVALID_FILE_ATTRIBUTES,
		};

		let wide: Vec<u16> = path
			.as_os_str()
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();
		let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
		return attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_HIDDEN) != 0;
	}

	#[cfg(not(windows))]
	{
		path.file_name()
			.and_then(|n| n.to_str())
			.map(|n| n.starts_with('.'))
			.unwrap_or(false)
	}
}

/// Snapshot of filesystem metadata for a single entry.
///
/// `inode` is populated on Unix and on Windows NTFS, where it is the 64-bit
/// File ID. FAT32 and exFAT leave it None, so change detection uses
/// `(inode, mtime, size)` where it can and falls back to the path where it
/// cannot.
#[derive(Debug, Clone)]
pub struct EntryMetadata {
	pub path: PathBuf,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<std::time::SystemTime>,
	pub accessed: Option<std::time::SystemTime>,
	pub created: Option<std::time::SystemTime>,
	pub inode: Option<u64>,
	pub permissions: Option<u32>,
	pub uid: Option<u32>,
	pub gid: Option<u32>,
	/// Where a symlink points, verbatim from `readlink`.
	pub link_target: Option<String>,
	pub is_hidden: bool,
}

impl From<DirEntry> for EntryMetadata {
	fn from(entry: DirEntry) -> Self {
		Self {
			path: entry.path.clone(),
			kind: entry.kind,
			size: entry.size,
			modified: entry.modified,
			accessed: entry.accessed,
			created: entry.created,
			inode: entry.inode,
			permissions: entry.permissions,
			uid: entry.uid,
			gid: entry.gid,
			link_target: entry.link_target,
			is_hidden: is_hidden_path(&entry.path),
		}
	}
}

/// Get platform-specific inode
///
/// On Unix/Linux/macOS, extracts the inode number directly from metadata.
/// On Windows NTFS, opens the file to retrieve the 64-bit File ID via GetFileInformationByHandle.
/// Returns None on FAT32/exFAT filesystems or when file access fails.
#[cfg(unix)]
pub fn get_inode(_path: &Path, metadata: &std::fs::Metadata) -> Option<u64> {
	use std::os::unix::fs::MetadataExt;
	Some(metadata.ino())
}

#[cfg(windows)]
pub fn get_inode(path: &Path, _metadata: &std::fs::Metadata) -> Option<u64> {
	use std::os::windows::ffi::OsStrExt;
	use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
	use windows_sys::Win32::Storage::FileSystem::{
		CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
		FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
		OPEN_EXISTING,
	};

	// Convert path to wide string for Windows API
	let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

	// Use CreateFileW with FILE_FLAG_BACKUP_SEMANTICS to allow opening directories.
	// std::fs::File::open fails for directories on Windows without this flag.
	let handle = unsafe {
		CreateFileW(
			wide_path.as_ptr(),
			GENERIC_READ,
			FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
			std::ptr::null_mut(),
			OPEN_EXISTING,
			FILE_FLAG_BACKUP_SEMANTICS, // Required to open directories
			0,
		)
	};

	if handle == INVALID_HANDLE_VALUE {
		tracing::debug!(
			"Failed to open path for File ID extraction: {}",
			path.display()
		);
		return None;
	}

	let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };

	let result = unsafe {
		if GetFileInformationByHandle(handle, &mut info) != 0 {
			// Combine high and low 32-bit values into 64-bit File ID
			let file_id = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);

			// File ID of 0 indicates FAT32/exFAT (no File ID support)
			if file_id == 0 {
				tracing::debug!(
					"File ID is 0 for {:?} (likely FAT32/exFAT filesystem)",
					path.file_name().unwrap_or_default()
				);
				None
			} else {
				tracing::trace!(
					"Extracted File ID: 0x{:016X} for {:?}",
					file_id,
					path.file_name().unwrap_or_default()
				);
				Some(file_id)
			}
		} else {
			// GetFileInformationByHandle failed
			// Common reasons: FAT32/exFAT filesystem, permission denied
			tracing::debug!(
				"GetFileInformationByHandle failed for {:?} (likely FAT32/exFAT or permission issue)",
				path.file_name().unwrap_or_default()
			);
			None
		}
	};

	// Always close the handle
	unsafe {
		CloseHandle(handle);
	}

	result
}

#[cfg(not(any(unix, windows)))]
pub fn get_inode(_path: &Path, _metadata: &std::fs::Metadata) -> Option<u64> {
	None
}

/// A file's birth time, asked of the kernel directly on Linux.
///
/// `std::fs::Metadata::created()` needs `statx`, which the standard library
/// does not reach on every libc, and a walk that silently stores `None` for
/// every file loses the one timestamp a copy cannot reproduce. `AT_SYMLINK_NOFOLLOW`
/// so a symlink reports its own birth, matching how everything else here
/// reads the link rather than the target.
#[cfg(target_os = "linux")]
pub fn birth_time(path: &Path, _metadata: &std::fs::Metadata) -> Option<std::time::SystemTime> {
	use std::os::unix::ffi::OsStrExt;

	// The kernel's struct statx, laid out by hand because the libc crate only
	// exposes it for glibc targets and the walk also ships as musl. The
	// kernel ABI is fixed at 256 bytes; only the fields up to the timestamps
	// are named, the rest is padding.
	#[repr(C)]
	struct StatxTimestamp {
		tv_sec: i64,
		tv_nsec: u32,
		__reserved: i32,
	}
	#[repr(C)]
	struct Statx {
		stx_mask: u32,
		stx_blksize: u32,
		stx_attributes: u64,
		stx_nlink: u32,
		stx_uid: u32,
		stx_gid: u32,
		stx_mode: u16,
		__spare0: u16,
		stx_ino: u64,
		stx_size: u64,
		stx_blocks: u64,
		stx_attributes_mask: u64,
		stx_atime: StatxTimestamp,
		stx_btime: StatxTimestamp,
		stx_ctime: StatxTimestamp,
		stx_mtime: StatxTimestamp,
		__padding: [u64; 14],
	}

	const STATX_BTIME: u32 = 0x800;

	let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
	let mut out: Statx = unsafe { std::mem::zeroed() };
	let rc = unsafe {
		libc::syscall(
			libc::SYS_statx,
			libc::AT_FDCWD,
			c_path.as_ptr(),
			libc::AT_SYMLINK_NOFOLLOW,
			STATX_BTIME,
			&mut out as *mut Statx,
		)
	};
	if rc != 0 || out.stx_mask & STATX_BTIME == 0 || out.stx_btime.tv_sec <= 0 {
		return None;
	}
	Some(
		std::time::UNIX_EPOCH
			+ std::time::Duration::new(out.stx_btime.tv_sec as u64, out.stx_btime.tv_nsec),
	)
}

#[cfg(not(target_os = "linux"))]
pub fn birth_time(_path: &Path, metadata: &std::fs::Metadata) -> Option<std::time::SystemTime> {
	metadata.created().ok()
}

/// Extracts filesystem metadata through either a volume backend or direct I/O.
///
/// Volume backends abstract cloud storage (S3, Dropbox) and local filesystems
/// behind a unified interface. When a backend is provided, metadata comes from
/// the volume's cache or API; otherwise this falls back to `tokio::fs` for local
/// paths. Cloud volumes MUST provide a backend since there's no local file to read.
///
/// Returns `Err` if the path doesn't exist or lacks read permissions. On permission
/// errors, the entry should still be indexed as inaccessible rather than skipped
/// entirely - this preserves the directory tree structure for UI navigation.
pub async fn extract_metadata(
	path: &Path,
	backend: Option<&std::sync::Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<EntryMetadata, std::io::Error> {
	if let Some(backend) = backend {
		let raw = backend
			.metadata(path)
			.await
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

		Ok(EntryMetadata {
			path: path.to_path_buf(),
			kind: raw.kind,
			size: raw.size,
			modified: raw.modified,
			accessed: raw.accessed,
			created: raw.created,
			inode: raw.inode,
			permissions: raw.permissions,
			uid: raw.uid,
			gid: raw.gid,
			link_target: raw.link_target,
			is_hidden: is_hidden_path(path),
		})
	} else {
		let metadata = tokio::fs::symlink_metadata(path).await?;

		let kind = if metadata.is_dir() {
			EntryKind::Directory
		} else if metadata.is_symlink() {
			EntryKind::Symlink
		} else {
			EntryKind::File
		};

		let inode = get_inode(path, &metadata);

		#[cfg(unix)]
		let (permissions, uid, gid) = {
			use std::os::unix::fs::MetadataExt;
			(
				Some(metadata.mode()),
				Some(metadata.uid()),
				Some(metadata.gid()),
			)
		};

		#[cfg(not(unix))]
		let (permissions, uid, gid) = (None, None, None);

		let link_target = if matches!(kind, EntryKind::Symlink) {
			tokio::fs::read_link(path)
				.await
				.ok()
				.map(|t| t.to_string_lossy().into_owned())
		} else {
			None
		};

		Ok(EntryMetadata {
			path: path.to_path_buf(),
			kind,
			size: metadata.len(),
			modified: metadata.modified().ok(),
			accessed: metadata.accessed().ok(),
			created: birth_time(path, &metadata),
			inode,
			permissions,
			uid,
			gid,
			link_target,
			is_hidden: is_hidden_path(path),
		})
	}
}
