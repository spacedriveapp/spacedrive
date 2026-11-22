//! Shared utilities for volume detection across platforms

use crate::{
	domain::volume::{SpacedriveVolumeId, SPACEDRIVE_VOLUME_ID_FILE},
	volume::{
		error::{VolumeError, VolumeResult},
		types::{FileSystem, VolumeFingerprint},
	},
};
use std::path::Path;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Parse size strings from df output (e.g., "1.5G", "931Gi", "1024K")
pub fn parse_size_string(size_str: &str) -> VolumeResult<u64> {
	if size_str == "-" {
		return Ok(0);
	}

	// Skip invalid size strings that don't look like numbers
	if size_str.is_empty() || size_str.chars().all(char::is_alphabetic) {
		return Ok(0);
	}

	let size_str = size_str.replace(",", ""); // Remove commas
	let (number_part, unit) = if let Some(pos) = size_str.find(char::is_alphabetic) {
		(&size_str[..pos], &size_str[pos..])
	} else {
		(size_str.as_str(), "")
	};

	let number: f64 = number_part
		.parse()
		.map_err(|_| VolumeError::InvalidData(format!("Invalid size: {}", size_str)))?;

	let multiplier = match unit.to_uppercase().as_str() {
		"" | "B" => 1,
		"K" | "KB" | "KI" => 1024,
		"M" | "MB" | "MI" => 1024 * 1024,
		"G" | "GB" | "GI" => 1024 * 1024 * 1024,
		"T" | "TB" | "TI" => 1024u64.pow(4),
		"P" | "PB" | "PI" => 1024u64.pow(5),
		_ => {
			warn!("Unknown size unit: {}", unit);
			1
		}
	};

	Ok((number * multiplier as f64) as u64)
}

/// Check if a filesystem should be considered a system filesystem
pub fn is_system_filesystem(filesystem: &str) -> bool {
	matches!(
		filesystem,
		"/" | "/dev" | "/proc" | "/sys" | "/tmp" | "/var/tmp"
	)
}

/// Check if a filesystem is virtual (not backed by physical storage)
pub fn is_virtual_filesystem(filesystem: &str) -> bool {
	let fs_lower = filesystem.to_lowercase();
	matches!(
		fs_lower.as_str(),
		"devfs"
			| "sysfs" | "proc"
			| "tmpfs" | "ramfs"
			| "devtmpfs"
			| "overlay"
			| "overlayfs"
			| "fuse" | "fusectl"
			| "cgroup"
			| "cgroup2"
			| "squashfs"
			| "efivarfs"
			| "pstore"
			| "bpf" | "debugfs"
			| "tracefs"
			| "configfs"
			| "securityfs"
			| "autofs"
			| "binfmt_misc"
			| "mqueue"
			| "hugetlbfs"
			| "rpc_pipefs"
			| "nsfs" | "selinuxfs"
			| "pipefs"
			| "sockfs"
	) || fs_lower.starts_with("map ")
		|| fs_lower.contains("auto_")
}

/// Check if a Linux mount point is a known OS/system path that should be
/// hidden from the user-visible volume list.
///
/// Covers the standard Linux filesystem hierarchy plus TrueNAS Scale's
/// additional split-root datasets (it uses separate ZFS datasets for
/// `/usr`, `/var`, `/etc`, etc. to support atomic updates).
#[cfg(target_os = "linux")]
pub fn is_system_mount_point(mount_point: &Path) -> bool {
	let path_str = mount_point.to_string_lossy();

	// Exact matches for system paths
	matches!(
		path_str.as_ref(),
		"/" | "/usr"
			| "/var"
			| "/etc"
			| "/opt"
			| "/srv"
			| "/root"
			| "/boot"
			| "/home"
			| "/run"
			| "/dev"
			| "/proc"
			| "/sys"
			| "/tmp"
			| "/audit"
			| "/data"
			| "/conf"
			| "/mnt"
			| "/lost+found"
	) ||
	// System subpaths
	path_str.starts_with("/boot/")
		|| path_str.starts_with("/sys/")
		|| path_str.starts_with("/proc/")
		|| path_str.starts_with("/dev/")
		|| (path_str.starts_with("/run/") && !path_str.starts_with("/run/media/"))
		|| path_str.starts_with("/var/log")
		|| path_str.starts_with("/var/db/")
		|| path_str.starts_with("/var/lib/systemd")
		|| path_str.starts_with("/var/local/")
		|| path_str.starts_with("/var/cache/")
}

/// Check if a mount point looks like a nested app/container volume that
/// shouldn't be exposed as a user-facing volume.
///
/// Catches Docker/Kubernetes layer mounts, snap packages, ZFS snapshot
/// directories, and TrueNAS Scale's iX Apps (each app creates many
/// nested ZFS datasets under `ix-applications/` — and the root of that
/// hierarchy itself should also be hidden).
#[cfg(target_os = "linux")]
pub fn is_nested_app_mount(mount_point: &Path) -> bool {
	let path_str = mount_point.to_string_lossy();

	// Match root directories as well as anything under them
	let is_under = |needle: &str| -> bool {
		path_str.contains(&format!("/{needle}/")) || path_str.ends_with(&format!("/{needle}"))
	};

	is_under(".ix-apps")
		|| is_under("ix-applications")
		|| path_str.contains("/docker/overlay2/")
		|| path_str.contains("/containerd/")
		|| path_str.contains("/kubelet/")
		|| path_str.contains("/snap/")
		|| path_str.contains("/.snapshots/")
		|| path_str.contains("/.zfs/snapshot/")
}

/// Re-evaluate whether a mount path should be hidden from the user based on
/// current platform visibility rules. Shared between the volume list query
/// and the library statistics calculation so both consistently hide system
/// mounts and nested app/container volumes even when the tracked DB row
/// predates these filters (and so has a stale `is_user_visible = true`).
#[cfg(target_os = "linux")]
pub fn should_hide_by_mount_path(mount_point: &Path) -> bool {
	is_system_mount_point(mount_point) || is_nested_app_mount(mount_point)
}

#[cfg(not(target_os = "linux"))]
pub fn should_hide_by_mount_path(_mount_point: &Path) -> bool {
	false
}

/// Parse filesystem type from string to FileSystem enum
pub fn parse_filesystem_type(fs_type: &str) -> FileSystem {
	match fs_type.to_lowercase().as_str() {
		"apfs" => FileSystem::APFS,
		"btrfs" => FileSystem::Btrfs,
		"zfs" => FileSystem::ZFS,
		"refs" => FileSystem::ReFS,
		"ntfs" => FileSystem::NTFS,
		"ext2" | "ext3" | "ext4" => FileSystem::Ext4,
		"xfs" => FileSystem::Other("XFS".to_string()),
		"fat32" | "vfat" => FileSystem::FAT32,
		"exfat" => FileSystem::ExFAT,
		"hfs+" | "hfsplus" => FileSystem::Other("HFS+".to_string()),
		_ => FileSystem::Other(fs_type.to_string()),
	}
}

/// Read or create .spacedrive-volume-id file on external volumes (async version)
/// This file provides persistent identification for removable drives
/// Returns the UUID from the file, or None if the volume is read-only
pub async fn read_or_create_dotfile(
	mount_point: &Path,
	device_id: Uuid,
	library_id: Option<Uuid>,
) -> Option<Uuid> {
	let id_file_path = mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

	// Try to read existing dotfile
	if let Ok(content) = tokio::fs::read_to_string(&id_file_path).await {
		if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
			debug!(
				"Found existing dotfile ID: {} at {}",
				spacedrive_id.id,
				id_file_path.display()
			);
			return Some(spacedrive_id.id);
		}
	}

	// Try to create new dotfile (if writable)
	if !mount_point.exists() {
		return None;
	}

	let spacedrive_id = SpacedriveVolumeId {
		id: Uuid::new_v4(),
		created: chrono::Utc::now(),
		device_name: None,
		volume_name: mount_point
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "Unknown".to_string()),
		device_id,
		library_id: library_id.unwrap_or(Uuid::nil()),
	};

	if let Ok(content) = serde_json::to_string_pretty(&spacedrive_id) {
		match tokio::fs::write(&id_file_path, content).await {
			Ok(()) => {
				info!(
					"Created dotfile with ID: {} at {}",
					spacedrive_id.id,
					id_file_path.display()
				);
				return Some(spacedrive_id.id);
			}
			Err(e) => {
				warn!(
					"Could not write volume identity dotfile to {}: {} — caller must fall back to filesystem-derived identity",
					id_file_path.display(),
					e
				);
			}
		}
	}

	None
}

/// Read or create .spacedrive-volume-id file on external volumes (sync version)
/// This file provides persistent identification for removable drives
/// Returns the UUID from the file, or None if the volume is read-only
pub fn read_or_create_dotfile_sync(
	mount_point: &Path,
	device_id: Uuid,
	library_id: Option<Uuid>,
) -> Option<Uuid> {
	let id_file_path = mount_point.join(SPACEDRIVE_VOLUME_ID_FILE);

	// Try to read existing dotfile
	if let Ok(content) = std::fs::read_to_string(&id_file_path) {
		if let Ok(spacedrive_id) = serde_json::from_str::<SpacedriveVolumeId>(&content) {
			debug!(
				"Found existing dotfile ID: {} at {}",
				spacedrive_id.id,
				id_file_path.display()
			);
			return Some(spacedrive_id.id);
		}
	}

	// Try to create new dotfile (if writable)
	if !mount_point.exists() {
		return None;
	}

	let spacedrive_id = SpacedriveVolumeId {
		id: Uuid::new_v4(),
		created: chrono::Utc::now(),
		device_name: None,
		volume_name: mount_point
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "Unknown".to_string()),
		device_id,
		library_id: library_id.unwrap_or(Uuid::nil()),
	};

	if let Ok(content) = serde_json::to_string_pretty(&spacedrive_id) {
		match std::fs::write(&id_file_path, content) {
			Ok(()) => {
				info!(
					"Created dotfile with ID: {} at {}",
					spacedrive_id.id,
					id_file_path.display()
				);
				return Some(spacedrive_id.id);
			}
			Err(e) => {
				warn!(
					"Could not write volume identity dotfile to {}: {} — caller must fall back to filesystem-derived identity",
					id_file_path.display(),
					e
				);
			}
		}
	}

	None
}

/// Derive a fingerprint for an external volume that has no readable or
/// writable identity dotfile (read-only media, full disk). Prefers the
/// filesystem's own UUID when the platform exposes one; otherwise falls
/// back to coarse volume metadata, which has weaker uniqueness (see
/// `VolumeFingerprint::from_external_metadata`). Both derivations depend
/// only on the volume itself, so the fingerprint stays stable across
/// machines and remounts.
pub fn external_volume_fallback_fingerprint(
	mount_point: &Path,
	block_device: Option<&str>,
	file_system: &FileSystem,
	total_capacity: u64,
) -> VolumeFingerprint {
	if let Some(fs_uuid) = filesystem_uuid(mount_point, block_device) {
		info!(
			"Using filesystem UUID {} as fallback identity for volume at {}",
			fs_uuid,
			mount_point.display()
		);
		return VolumeFingerprint::from_external_fs_uuid(&fs_uuid);
	}

	let root_created_unix_secs = std::fs::metadata(mount_point)
		.ok()
		.and_then(|meta| meta.created().ok())
		.and_then(|created| created.duration_since(std::time::UNIX_EPOCH).ok())
		.map(|duration| duration.as_secs() as i64);

	warn!(
		"No filesystem UUID available for volume at {}; using metadata-based fallback fingerprint (weaker uniqueness)",
		mount_point.display()
	);
	VolumeFingerprint::from_external_metadata(
		file_system.as_str(),
		total_capacity,
		root_created_unix_secs,
	)
}

/// Look up the filesystem UUID for a mounted volume via `diskutil info`.
#[cfg(target_os = "macos")]
fn filesystem_uuid(mount_point: &Path, _block_device: Option<&str>) -> Option<String> {
	let output = std::process::Command::new("diskutil")
		.args(["info", "-plist"])
		.arg(mount_point)
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}

	let plist = String::from_utf8_lossy(&output.stdout);
	let key_pos = plist.find("<key>VolumeUUID</key>")?;
	let rest = &plist[key_pos..];
	let value_start = rest.find("<string>")? + "<string>".len();
	let value_end = rest[value_start..].find("</string>")? + value_start;
	let uuid = rest[value_start..value_end].trim();
	(!uuid.is_empty()).then(|| uuid.to_string())
}

/// Look up the filesystem UUID by matching the volume's block device
/// against the `/dev/disk/by-uuid` symlinks.
#[cfg(target_os = "linux")]
fn filesystem_uuid(_mount_point: &Path, block_device: Option<&str>) -> Option<String> {
	let canonical_device = std::fs::canonicalize(block_device?).ok()?;
	for entry in std::fs::read_dir("/dev/disk/by-uuid").ok()?.flatten() {
		if let Ok(target) = std::fs::canonicalize(entry.path()) {
			if target == canonical_device {
				return entry.file_name().to_str().map(str::to_string);
			}
		}
	}
	None
}

/// Platforms without a filesystem UUID lookup use the metadata fallback.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn filesystem_uuid(_mount_point: &Path, _block_device: Option<&str>) -> Option<String> {
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_size_string() {
		assert_eq!(parse_size_string("1024").unwrap(), 1024);
		assert_eq!(parse_size_string("1K").unwrap(), 1024);
		assert_eq!(parse_size_string("1M").unwrap(), 1024 * 1024);
		assert_eq!(parse_size_string("1G").unwrap(), 1024 * 1024 * 1024);
		assert_eq!(
			parse_size_string("1.5G").unwrap(),
			(1.5 * 1024.0 * 1024.0 * 1024.0) as u64
		);
		assert_eq!(parse_size_string("-").unwrap(), 0);
	}

	#[test]
	fn test_filesystem_detection() {
		assert!(is_virtual_filesystem("tmpfs"));
		assert!(is_virtual_filesystem("proc"));
		assert!(!is_virtual_filesystem("ext4"));

		assert!(is_system_filesystem("/"));
		assert!(is_system_filesystem("/proc"));
		assert!(!is_system_filesystem("/home"));
	}

	#[test]
	fn test_parse_filesystem_type() {
		assert!(matches!(parse_filesystem_type("apfs"), FileSystem::APFS));
		assert!(matches!(parse_filesystem_type("btrfs"), FileSystem::Btrfs));
		assert!(matches!(parse_filesystem_type("ext4"), FileSystem::Ext4));
		assert!(matches!(
			parse_filesystem_type("unknown"),
			FileSystem::Other(_)
		));
	}

	#[cfg(target_os = "linux")]
	#[test]
	fn test_should_hide_by_mount_path_linux() {
		// System mounts — always hidden
		assert!(should_hide_by_mount_path(Path::new("/")));
		assert!(should_hide_by_mount_path(Path::new("/usr")));
		assert!(should_hide_by_mount_path(Path::new("/var")));
		assert!(should_hide_by_mount_path(Path::new("/etc")));
		assert!(should_hide_by_mount_path(Path::new("/home")));
		assert!(should_hide_by_mount_path(Path::new("/boot/grub")));
		assert!(should_hide_by_mount_path(Path::new("/var/log/journal")));
		assert!(should_hide_by_mount_path(Path::new(
			"/var/db/system/netdata"
		)));
		assert!(should_hide_by_mount_path(Path::new("/sys/firmware/efi")));
		assert!(should_hide_by_mount_path(Path::new("/run/user/1000/gvfs")));

		// TrueNAS Scale app-managed datasets
		assert!(should_hide_by_mount_path(Path::new("/mnt/.ix-apps")));
		assert!(should_hide_by_mount_path(Path::new("/mnt/.ix-apps/docker")));
		assert!(should_hide_by_mount_path(Path::new(
			"/mnt/pool/ix-applications"
		)));
		assert!(should_hide_by_mount_path(Path::new(
			"/mnt/pool/ix-applications/releases/plex/volumes/ix_volumes/ix-plex_data"
		)));

		// User data — not hidden
		assert!(!should_hide_by_mount_path(Path::new("/mnt/pool")));
		assert!(!should_hide_by_mount_path(Path::new("/mnt/pool/footage")));
		assert!(!should_hide_by_mount_path(Path::new(
			"/mnt/pool/calvin-nas"
		)));

		// udisks2 removable drives on Fedora and Arch
		assert!(!should_hide_by_mount_path(Path::new(
			"/run/media/alice/USB"
		)));
	}
}

/// Whether `path` is where a filesystem is mounted right now.
///
/// An unmounted drive leaves its mount point behind as an empty directory on
/// the parent filesystem, and that directory passes every existence check.
/// On Linux what tells the two apart is the device: a mount point's device
/// differs from its parent's, or it is the filesystem root. macOS cannot use
/// that test, because APFS reports the sealed system volume's device for the
/// Data volume and every firmlink into it, so `/System/Volumes/Data` stats
/// the same as `/`. There the kernel's own answer is used: `statfs` names
/// the mount a path lives on, and a mount point is a path that names itself.
/// Windows answers the same
/// question through `GetVolumePathNameW`, which names the mount point of the
/// volume holding a path: a path that is its own volume root is a mount point
/// (a drive letter or a folder a volume is mounted at), while the directory a
/// drive left behind resolves to the parent volume's root. Platforms with
/// neither answer by existence, which is the check this replaces.
pub fn is_mount_point(path: &Path) -> bool {
	#[cfg(any(target_os = "macos", target_os = "ios"))]
	{
		is_mount_point_by_statfs(path, statfs_mounted_on)
	}
	#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
	{
		use std::os::unix::fs::MetadataExt;
		let Ok(meta) = std::fs::metadata(path) else {
			return false;
		};
		let Some(parent) = path.parent() else {
			return true;
		};
		match std::fs::metadata(parent) {
			Ok(parent_meta) => parent_meta.dev() != meta.dev(),
			Err(_) => true,
		}
	}
	#[cfg(windows)]
	{
		use std::os::windows::ffi::{OsStrExt, OsStringExt};
		use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;

		if !path.is_dir() {
			return false;
		}
		// The path as given, not canonicalized: GetVolumePathNameW judges the
		// string it is handed, and a subst or mapped drive letter resolves
		// through canonicalize to the target it stands for, which would make
		// the drive's own root read as not a mount point.
		let wide: Vec<u16> = path
			.as_os_str()
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();
		let mut root = vec![0u16; 1024];
		if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
			return false;
		}
		let len = root.iter().position(|&c| c == 0).unwrap_or(root.len());
		let root = std::ffi::OsString::from_wide(&root[..len]);
		let strip = |s: &std::ffi::OsStr| {
			s.to_string_lossy()
				.trim_start_matches(r"\\?\")
				.trim_end_matches(['\\', '/'])
				.to_ascii_lowercase()
		};
		strip(&root) == strip(path.as_os_str())
	}
	#[cfg(not(any(unix, windows)))]
	{
		path.exists()
	}
}

/// Whether `path` is the mount point `mounted_on` reports for it.
///
/// The path may reach the mount through a symlink, such as `/tmp` for
/// `/private/tmp`, while the kernel names the mount by its real path, so the
/// resolved path is compared as well as the one given. A path the lookup
/// cannot answer for is not a mount point, matching a missing directory.
#[cfg(any(target_os = "macos", target_os = "ios", test))]
fn is_mount_point_by_statfs(
	path: &Path,
	mounted_on: impl Fn(&Path) -> std::io::Result<std::path::PathBuf>,
) -> bool {
	let Ok(mount) = mounted_on(path) else {
		return false;
	};
	mount == path || std::fs::canonicalize(path).is_ok_and(|resolved| mount == resolved)
}

/// The mount point of the filesystem holding `path`, from `statfs`'s
/// `f_mntonname`.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn statfs_mounted_on(path: &Path) -> std::io::Result<std::path::PathBuf> {
	use std::os::unix::ffi::{OsStrExt, OsStringExt};

	let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
		.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
	let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
	if unsafe { libc::statfs(c_path.as_ptr(), &mut stat) } != 0 {
		return Err(std::io::Error::last_os_error());
	}
	let name: Vec<u8> = stat
		.f_mntonname
		.iter()
		.take_while(|&&c| c != 0)
		.map(|&c| c as u8)
		.collect();
	Ok(std::ffi::OsString::from_vec(name).into())
}

#[cfg(test)]
mod statfs_mount_point_tests {
	use super::is_mount_point_by_statfs;
	use std::{
		io,
		path::{Path, PathBuf},
	};

	/// What `statfs` reports on macOS 26: the sealed system volume at `/`,
	/// the Data volume at `/System/Volumes/Data` with `/Users` firmlinked
	/// into it, and an external drive at `/Volumes/Drive`. `/Volumes/Gone`
	/// is the directory an ejected drive left behind on the Data volume.
	fn macos_26(path: &Path) -> io::Result<PathBuf> {
		let on = |mount: &str| Ok(PathBuf::from(mount));
		let path = path.to_string_lossy();
		let path = path.trim_end_matches('/');
		match path {
			"" => on("/"),
			"/System/Volumes/Data" => on("/System/Volumes/Data"),
			"/Volumes/Drive" => on("/Volumes/Drive"),
			p if p.starts_with("/Volumes/Drive/") => on("/Volumes/Drive"),
			p if p.starts_with("/System/Volumes/Data/")
				|| p.starts_with("/Users")
				|| p.starts_with("/Volumes")
				|| p.starts_with("/private") =>
			{
				on("/System/Volumes/Data")
			}
			"/System" | "/System/Volumes" => on("/"),
			_ => Err(io::Error::from(io::ErrorKind::NotFound)),
		}
	}

	fn mount_point(path: &str) -> bool {
		is_mount_point_by_statfs(Path::new(path), macos_26)
	}

	/// The Data volume shares its device number with `/` and its parent,
	/// which is what made a device comparison call it unmounted.
	#[test]
	fn the_apfs_data_volume_is_a_mount_point() {
		assert!(mount_point("/"));
		assert!(mount_point("/System/Volumes/Data"));
		assert!(mount_point("/System/Volumes/Data/"));
		assert!(!mount_point("/System/Volumes"));
	}

	/// A firmlink reaches the Data volume without being where it is mounted.
	#[test]
	fn a_firmlinked_folder_is_not_a_mount_point() {
		assert!(!mount_point("/Users"));
		assert!(!mount_point("/Users/me/Projects"));
		assert!(!mount_point("/System/Volumes/Data/Users/me"));
		assert!(!mount_point("/private/var/folders"));
	}

	/// An external drive is a mount point while mounted; the directory it
	/// leaves behind belongs to the Data volume and is not.
	#[test]
	fn an_ejected_drive_leaves_a_directory_that_is_not_a_mount_point() {
		assert!(mount_point("/Volumes/Drive"));
		assert!(!mount_point("/Volumes/Drive/photos"));
		assert!(!mount_point("/Volumes/Gone"));
	}

	/// A path `statfs` cannot answer for is not a mount point.
	#[test]
	fn a_path_statfs_fails_on_is_not_a_mount_point() {
		assert!(!mount_point("/nowhere"));
	}

	/// A path reached through a symlink is compared by where it resolves,
	/// because the kernel names the mount by its real path.
	#[cfg(unix)]
	#[test]
	fn a_symlink_to_a_mount_point_is_one() {
		let dir = tempfile::tempdir().unwrap();
		let mount = std::fs::canonicalize(dir.path()).unwrap().join("mnt");
		std::fs::create_dir(&mount).unwrap();
		let link = dir.path().join("link");
		std::os::unix::fs::symlink(&mount, &link).unwrap();
		let reported = mount.clone();
		let statfs = move |_: &Path| Ok(reported.clone());
		assert!(is_mount_point_by_statfs(&link, &statfs));
		assert!(is_mount_point_by_statfs(&mount, &statfs));
		assert!(!is_mount_point_by_statfs(
			&dir.path().join("missing"),
			&statfs
		));
	}
}

#[cfg(all(test, windows))]
mod mount_point_tests {
	use super::is_mount_point;
	use std::path::Path;

	/// The system drive is a mount point, a directory on it is not, and a
	/// missing path is not.
	#[test]
	fn a_drive_root_is_a_mount_point_and_a_folder_on_it_is_not() {
		let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
		assert!(is_mount_point(Path::new(&format!("{system}\\"))));
		let dir = tempfile::tempdir().unwrap();
		let left_behind = dir.path().join("mount");
		std::fs::create_dir(&left_behind).unwrap();
		assert!(!is_mount_point(&left_behind));
		assert!(!is_mount_point(&dir.path().join("missing")));
	}
}

#[cfg(all(test, unix))]
mod mount_point_tests {
	use super::is_mount_point;
	use std::path::Path;

	/// The filesystem root is a mount point, a directory on the same device as
	/// its parent is not, and a missing path is not.
	#[test]
	fn a_mount_point_differs_from_its_parent_by_device() {
		assert!(is_mount_point(Path::new("/")));
		let dir = tempfile::tempdir().unwrap();
		let left_behind = dir.path().join("mount");
		std::fs::create_dir(&left_behind).unwrap();
		assert!(!is_mount_point(&left_behind));
		assert!(!is_mount_point(&dir.path().join("missing")));
	}
}
