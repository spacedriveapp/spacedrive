//! Filesystem attributes a path carries: the permission mode, the
//! modification time, and whether it is hidden.
//!
//! Hidden is a flag on macOS (`UF_HIDDEN`) and Windows
//! (`FILE_ATTRIBUTE_HIDDEN`); elsewhere it is a leading dot in the name,
//! which is a rename rather than an attribute, so setting it there is
//! refused.

use std::{
	io,
	path::Path,
	time::{Duration, UNIX_EPOCH},
};

use crate::infra::job::journal::Attributes;

/// The attributes as the filesystem holds them now.
pub async fn read(path: &Path) -> io::Result<Attributes> {
	let meta = tokio::fs::symlink_metadata(path).await?;
	Ok(Attributes {
		mode: mode_of(&meta),
		modified_ms: meta
			.modified()
			.ok()
			.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
			.map(|elapsed| elapsed.as_millis() as i64),
		hidden: Some(hidden_of(path, &meta)),
	})
}

/// Set each attribute the change names.
pub async fn apply(path: &Path, attributes: &Attributes) -> io::Result<()> {
	if let Some(mode) = attributes.mode {
		set_mode(path, mode).await?;
	}
	if let Some(modified_ms) = attributes.modified_ms {
		let time = UNIX_EPOCH + Duration::from_millis(modified_ms.max(0) as u64);
		let file = std::fs::OpenOptions::new()
			.write(true)
			.open(path)
			.or_else(|_| std::fs::File::open(path))?;
		file.set_modified(time)?;
	}
	if let Some(hidden) = attributes.hidden {
		set_hidden(path, hidden).await?;
	}
	Ok(())
}

/// Whether any named attribute differs from what the path carries.
pub fn differs(current: &Attributes, wanted: &Attributes) -> bool {
	wanted.mode.is_some_and(|mode| current.mode != Some(mode))
		|| wanted
			.modified_ms
			.is_some_and(|ms| current.modified_ms != Some(ms))
		|| wanted
			.hidden
			.is_some_and(|hidden| current.hidden != Some(hidden))
}

/// The attributes that change: each named one that differs, the rest
/// absent.
pub fn changing(current: &Attributes, wanted: &Attributes) -> Attributes {
	Attributes {
		mode: wanted.mode.filter(|mode| current.mode != Some(*mode)),
		modified_ms: wanted
			.modified_ms
			.filter(|ms| current.modified_ms != Some(*ms)),
		hidden: wanted
			.hidden
			.filter(|hidden| current.hidden != Some(*hidden)),
	}
}

/// Whether the filesystem holding `path` can carry each attribute.
pub fn supported(
	file_system: &crate::volume::types::FileSystem,
	attributes: &Attributes,
) -> Result<(), String> {
	use crate::volume::types::FileSystem;
	if attributes.mode.is_some() && matches!(file_system, FileSystem::FAT32 | FileSystem::ExFAT) {
		return Err(format!("{file_system} has no permission bits"));
	}
	if attributes.hidden.is_some() && !cfg!(any(target_os = "macos", target_os = "windows")) {
		return Err("hidden is a leading dot on this platform, which is a rename".to_string());
	}
	Ok(())
}

#[cfg(unix)]
fn mode_of(meta: &std::fs::Metadata) -> Option<u32> {
	use std::os::unix::fs::PermissionsExt;
	Some(meta.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn mode_of(_meta: &std::fs::Metadata) -> Option<u32> {
	None
}

#[cfg(unix)]
async fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
	use std::os::unix::fs::PermissionsExt;
	tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await
}

#[cfg(not(unix))]
async fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"this platform has no permission mode",
	))
}

#[cfg(target_os = "macos")]
fn hidden_of(_path: &Path, meta: &std::fs::Metadata) -> bool {
	use std::os::macos::fs::MetadataExt;
	meta.st_flags() & libc::UF_HIDDEN != 0
}

#[cfg(target_os = "macos")]
async fn set_hidden(path: &Path, hidden: bool) -> io::Result<()> {
	use std::os::macos::fs::MetadataExt;
	let meta = tokio::fs::symlink_metadata(path).await?;
	let flags = if hidden {
		meta.st_flags() | libc::UF_HIDDEN
	} else {
		meta.st_flags() & !libc::UF_HIDDEN
	};
	let owned = path.to_path_buf();
	tokio::task::spawn_blocking(move || {
		let c_path = std::ffi::CString::new(owned.as_os_str().as_encoded_bytes())
			.map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL in the path"))?;
		if unsafe { libc::chflags(c_path.as_ptr(), flags) } == 0 {
			Ok(())
		} else {
			Err(io::Error::last_os_error())
		}
	})
	.await
	.map_err(|error| io::Error::other(error.to_string()))?
}

#[cfg(windows)]
fn hidden_of(_path: &Path, meta: &std::fs::Metadata) -> bool {
	use std::os::windows::fs::MetadataExt;
	meta.file_attributes() & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(windows)]
async fn set_hidden(path: &Path, hidden: bool) -> io::Result<()> {
	use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
	use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
	let meta = tokio::fs::symlink_metadata(path).await?;
	let attributes = if hidden {
		meta.file_attributes() | FILE_ATTRIBUTE_HIDDEN
	} else {
		meta.file_attributes() & !FILE_ATTRIBUTE_HIDDEN
	};
	let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
	if unsafe { SetFileAttributesW(wide.as_ptr(), attributes) } == 0 {
		return Err(io::Error::last_os_error());
	}
	Ok(())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn hidden_of(path: &Path, _meta: &std::fs::Metadata) -> bool {
	path.file_name()
		.and_then(|name| name.to_str())
		.is_some_and(|name| name.starts_with('.'))
}

#[cfg(not(any(target_os = "macos", windows)))]
async fn set_hidden(_path: &Path, _hidden: bool) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"hidden is a leading dot on this platform",
	))
}
