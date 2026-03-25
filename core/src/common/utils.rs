//! Shared utility functions
//
// Note: Device ID management has been moved to device::manager for better
// module organization. Import from there instead:
// use crate::device::manager::{get_current_device_id, set_current_device_id};

/// Strip Windows extended path prefixes produced by `std::fs::canonicalize()`.
///
/// On Windows, `canonicalize()` returns paths like `\\?\C:\...` (local) or
/// `\\?\UNC\server\share\...` (network). These prefixes break `starts_with()`
/// matching throughout the codebase and must be normalized.
///
/// - `\\?\UNC\server\share\...` → `\\server\share\...`
/// - `\\?\C:\...` → `C:\...`
/// - All other paths are returned unchanged.
#[cfg(windows)]
pub fn strip_windows_extended_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
	if let Some(s) = path.to_str() {
		if s.starts_with(r"\\?\UNC\") {
			// \\?\UNC\server\share\... → \\server\share\...
			std::path::PathBuf::from(format!(r"\\{}", &s[8..]))
		} else if let Some(stripped) = s.strip_prefix(r"\\?\") {
			// Only strip \\?\ when followed by a drive letter (e.g. C:\).
			// Leave volume GUIDs (\\?\Volume{...}\) and other verbatim
			// forms untouched — they are invalid without the prefix.
			if stripped.as_bytes().get(1) == Some(&b':') {
				std::path::PathBuf::from(stripped)
			} else {
				path
			}
		} else {
			path
		}
	} else {
		path
	}
}

/// No-op on non-Windows platforms.
#[cfg(not(windows))]
pub fn strip_windows_extended_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
	path
}
