//! Platform-specific TRIM and hole punching for secure deletion
//!
//! TRIM operations notify SSDs that data blocks are no longer in use,
//! allowing the drive's controller to garbage collect them. This is more
//! effective than overwriting on SSDs due to wear-leveling.
//!
//! Hole punching (FALLOC_FL_PUNCH_HOLE) deallocates storage space while
//! preserving the file, making the data unrecoverable on supporting filesystems.

use std::path::Path;
use tracing::{debug, warn};

/// Result of a TRIM/hole punch operation
#[derive(Debug, Clone)]
pub struct TrimResult {
	pub success: bool,
	pub bytes_trimmed: u64,
	pub error: Option<String>,
}

impl TrimResult {
	fn success(bytes: u64) -> Self {
		Self {
			success: true,
			bytes_trimmed: bytes,
			error: None,
		}
	}

	fn error(msg: impl Into<String>) -> Self {
		Self {
			success: false,
			bytes_trimmed: 0,
			error: Some(msg.into()),
		}
	}

	fn unsupported(reason: &str) -> Self {
		Self {
			success: false,
			bytes_trimmed: 0,
			error: Some(format!("TRIM not supported: {}", reason)),
		}
	}
}

/// Attempt to TRIM/hole punch a file to securely deallocate its storage.
/// Falls back gracefully if not supported on the platform or filesystem.
pub async fn trim_file(path: &Path) -> TrimResult {
	#[cfg(target_os = "macos")]
	{
		trim_file_macos(path).await
	}

	#[cfg(target_os = "linux")]
	{
		trim_file_linux(path).await
	}

	#[cfg(target_os = "windows")]
	{
		trim_file_windows(path).await
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		TrimResult::unsupported("platform not supported")
	}
}

/// Check if TRIM is likely supported for a given path.
/// Returns true if the underlying storage supports TRIM operations.
pub async fn is_trim_supported(path: &Path) -> bool {
	#[cfg(target_os = "macos")]
	{
		is_trim_supported_macos(path).await
	}

	#[cfg(target_os = "linux")]
	{
		is_trim_supported_linux(path).await
	}

	#[cfg(target_os = "windows")]
	{
		is_trim_supported_windows(path).await
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		false
	}
}

// =============================================================================
// macOS Implementation
// =============================================================================

#[cfg(target_os = "macos")]
async fn trim_file_macos(path: &Path) -> TrimResult {
	use std::os::unix::io::AsRawFd;
	use tokio::fs;

	let file = match fs::OpenOptions::new().write(true).open(path).await {
		Ok(f) => f,
		Err(e) => return TrimResult::error(format!("Failed to open file: {}", e)),
	};

	let metadata = match fs::metadata(path).await {
		Ok(m) => m,
		Err(e) => return TrimResult::error(format!("Failed to get metadata: {}", e)),
	};

	let size = metadata.len();
	let std_file = file.into_std().await;
	let fd = std_file.as_raw_fd();

	// Use F_PUNCHHOLE to deallocate file blocks (macOS 10.12+)
	// This punches a hole in the file, deallocating the underlying storage
	let result = tokio::task::spawn_blocking(move || {
		// fpunchhole_t structure for F_PUNCHHOLE
		#[repr(C)]
		struct FPunchHole {
			fp_flags: libc::c_uint,
			reserved: libc::c_uint,
			fp_offset: libc::off_t,
			fp_length: libc::off_t,
		}

		let punch_hole = FPunchHole {
			fp_flags: 0,
			reserved: 0,
			fp_offset: 0,
			fp_length: size as libc::off_t,
		};

		// F_PUNCHHOLE = 99 on macOS
		const F_PUNCHHOLE: libc::c_int = 99;

		let ret = unsafe {
			libc::fcntl(
				fd,
				F_PUNCHHOLE,
				&punch_hole as *const FPunchHole as *const libc::c_void,
			)
		};

		if ret == 0 {
			debug!("Successfully punched hole in file: {} bytes", size);
			TrimResult::success(size)
		} else {
			let errno = std::io::Error::last_os_error();
			warn!("F_PUNCHHOLE failed: {}", errno);
			TrimResult::error(format!("F_PUNCHHOLE failed: {}", errno))
		}
	})
	.await;

	match result {
		Ok(r) => r,
		Err(e) => TrimResult::error(format!("Task join error: {}", e)),
	}
}

#[cfg(target_os = "macos")]
async fn is_trim_supported_macos(path: &Path) -> bool {
	use std::process::Command;

	// Check if file is on an APFS or HFS+ volume with TRIM support
	// Most SSDs on macOS support TRIM natively since macOS 10.10.4
	let output = tokio::task::spawn_blocking(move || {
		Command::new("diskutil")
			.args(["info", "-plist", "/"])
			.output()
	})
	.await;

	match output {
		Ok(Ok(output)) if output.status.success() => {
			let output_str = String::from_utf8_lossy(&output.stdout);
			// APFS and modern HFS+ volumes on SSDs support TRIM
			output_str.contains("APFS") || output_str.contains("SolidState")
		}
		_ => {
			// Default to true on macOS since most Macs have SSDs
			true
		}
	}
}

// =============================================================================
// Linux Implementation
// =============================================================================

#[cfg(target_os = "linux")]
async fn trim_file_linux(path: &Path) -> TrimResult {
	use std::os::unix::io::AsRawFd;
	use tokio::fs;

	let file = match fs::OpenOptions::new().write(true).open(path).await {
		Ok(f) => f,
		Err(e) => return TrimResult::error(format!("Failed to open file: {}", e)),
	};

	let metadata = match fs::metadata(path).await {
		Ok(m) => m,
		Err(e) => return TrimResult::error(format!("Failed to get metadata: {}", e)),
	};

	let size = metadata.len();
	let std_file = file.into_std().await;
	let fd = std_file.as_raw_fd();

	// Use fallocate with FALLOC_FL_PUNCH_HOLE to deallocate file blocks
	let result = tokio::task::spawn_blocking(move || {
		// FALLOC_FL_PUNCH_HOLE = 0x02, FALLOC_FL_KEEP_SIZE = 0x01
		const FALLOC_FL_PUNCH_HOLE: libc::c_int = 0x02;
		const FALLOC_FL_KEEP_SIZE: libc::c_int = 0x01;

		let ret = unsafe {
			libc::fallocate(
				fd,
				FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE,
				0,
				size as libc::off_t,
			)
		};

		if ret == 0 {
			debug!("Successfully punched hole in file: {} bytes", size);
			TrimResult::success(size)
		} else {
			let errno = std::io::Error::last_os_error();
			warn!("fallocate PUNCH_HOLE failed: {}", errno);
			TrimResult::error(format!("fallocate PUNCH_HOLE failed: {}", errno))
		}
	})
	.await;

	match result {
		Ok(r) => r,
		Err(e) => TrimResult::error(format!("Task join error: {}", e)),
	}
}

#[cfg(target_os = "linux")]
async fn is_trim_supported_linux(path: &Path) -> bool {
	use std::process::Command;

	// Check if the filesystem supports hole punching via /proc/mounts or similar
	let path_str = path.to_string_lossy().to_string();

	let result = tokio::task::spawn_blocking(move || {
		// Try to determine if TRIM is supported by checking the mount options
		// and filesystem type
		let output = Command::new("findmnt")
			.args(["-n", "-o", "FSTYPE,OPTIONS", "-T", &path_str])
			.output();

		match output {
			Ok(output) if output.status.success() => {
				let output_str = String::from_utf8_lossy(&output.stdout);
				// ext4, xfs, btrfs support hole punching
				output_str.contains("ext4")
					|| output_str.contains("xfs")
					|| output_str.contains("btrfs")
					|| output_str.contains("f2fs")
			}
			_ => {
				// Fallback: assume modern filesystems support it
				true
			}
		}
	})
	.await;

	result.unwrap_or(false)
}

// =============================================================================
// Windows Implementation
// =============================================================================

#[cfg(target_os = "windows")]
async fn trim_file_windows(path: &Path) -> TrimResult {
	use std::os::windows::io::AsRawHandle;
	use tokio::fs;
	use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
	use windows_sys::Win32::Storage::FileSystem::{
		CreateFileW, DeviceIoControl, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
	};
	use windows_sys::Win32::System::Ioctl::FSCTL_FILE_LEVEL_TRIM;

	let metadata = match fs::metadata(path).await {
		Ok(m) => m,
		Err(e) => return TrimResult::error(format!("Failed to get metadata: {}", e)),
	};

	let size = metadata.len();
	let path_clone = path.to_path_buf();

	let result = tokio::task::spawn_blocking(move || {
		// FILE_LEVEL_TRIM_RANGE structure
		#[repr(C)]
		struct FileLevelTrimRange {
			offset: u64,
			length: u64,
		}

		// FILE_LEVEL_TRIM_OUTPUT structure
		#[repr(C)]
		struct FileLevelTrimOutput {
			num_ranges_processed: u32,
		}

		// Convert path to wide string
		use std::os::windows::ffi::OsStrExt;
		let wide_path: Vec<u16> = path_clone
			.as_os_str()
			.encode_wide()
			.chain(std::iter::once(0))
			.collect();

		let handle = unsafe {
			CreateFileW(
				wide_path.as_ptr(),
				0x40000000, // GENERIC_WRITE
				FILE_SHARE_READ | FILE_SHARE_WRITE,
				std::ptr::null(),
				OPEN_EXISTING,
				0,
				std::ptr::null_mut() as HANDLE,
			)
		};

		if handle == -1isize as HANDLE {
			let err = std::io::Error::last_os_error();
			return TrimResult::error(format!("Failed to open file: {}", err));
		}

		let range = FileLevelTrimRange {
			offset: 0,
			length: size,
		};

		let mut output = FileLevelTrimOutput {
			num_ranges_processed: 0,
		};
		let mut bytes_returned: u32 = 0;

		let success = unsafe {
			DeviceIoControl(
				handle,
				FSCTL_FILE_LEVEL_TRIM,
				&range as *const _ as *const std::ffi::c_void,
				std::mem::size_of::<FileLevelTrimRange>() as u32,
				&mut output as *mut _ as *mut std::ffi::c_void,
				std::mem::size_of::<FileLevelTrimOutput>() as u32,
				&mut bytes_returned,
				std::ptr::null_mut(),
			)
		};

		unsafe {
			CloseHandle(handle);
		}

		if success != 0 {
			debug!("Successfully trimmed file: {} bytes", size);
			TrimResult::success(size)
		} else {
			let err = std::io::Error::last_os_error();
			warn!("FSCTL_FILE_LEVEL_TRIM failed: {}", err);
			TrimResult::error(format!("FSCTL_FILE_LEVEL_TRIM failed: {}", err))
		}
	})
	.await;

	match result {
		Ok(r) => r,
		Err(e) => TrimResult::error(format!("Task join error: {}", e)),
	}
}

#[cfg(target_os = "windows")]
async fn is_trim_supported_windows(path: &Path) -> bool {
	use std::process::Command;

	let path_str = path.to_string_lossy().to_string();

	// Extract drive letter from path
	let drive = if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
		path_str[..2].to_string()
	} else {
		return false;
	};

	let result = tokio::task::spawn_blocking(move || {
		// Use PowerShell to check if the drive is an SSD with TRIM support
		let output = Command::new("powershell")
			.args([
				"-Command",
				&format!(
					"$disk = Get-PhysicalDisk | Where-Object {{ $_.DeviceId -eq (Get-Partition -DriveLetter '{}').DiskNumber }}; $disk.MediaType",
					drive.chars().next().unwrap_or('C')
				),
			])
			.output();

		match output {
			Ok(output) if output.status.success() => {
				let output_str = String::from_utf8_lossy(&output.stdout);
				output_str.trim() == "SSD"
			}
			_ => {
				// Default to false on Windows
				false
			}
		}
	})
	.await;

	result.unwrap_or(false)
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::NamedTempFile;

	#[tokio::test]
	async fn test_trim_result_constructors() {
		let success = TrimResult::success(1024);
		assert!(success.success);
		assert_eq!(success.bytes_trimmed, 1024);
		assert!(success.error.is_none());

		let error = TrimResult::error("test error");
		assert!(!error.success);
		assert_eq!(error.bytes_trimmed, 0);
		assert!(error.error.is_some());

		let unsupported = TrimResult::unsupported("test reason");
		assert!(!unsupported.success);
		assert!(unsupported.error.unwrap().contains("TRIM not supported"));
	}

	#[tokio::test]
	async fn test_is_trim_supported() {
		// This test just verifies the function doesn't panic
		let temp_file = NamedTempFile::new().unwrap();
		let _ = is_trim_supported(temp_file.path()).await;
	}
}
