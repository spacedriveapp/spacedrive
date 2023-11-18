#![doc = include_str!("../README.md")]
#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	clippy::expect_used,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::as_conversions,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(unsafe_code, deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use std::{fs, path::PathBuf};

pub mod error;
use dirs::home_dir;
use error::Result;

pub struct DiskAccess;

impl DiskAccess {
	/// This function is a no-op on non-Unix systems.
	///
	/// This function checks if we have read and write access to a path.
	fn is_path_rw(path: PathBuf) -> bool {
		#[cfg(target_family = "unix")]
		use std::os::unix::fs::MetadataExt;

		(fs::metadata(path)).map_or(false, |md| {
			let mode = md.mode();
			mode & 0x180 == 0x180 // rw access
		})
	}

	/// This checks if a path is writeable or not. If not, it is read-only.
	#[must_use]
	pub fn is_path_writeable(path: PathBuf) -> bool {
		!fs::metadata(path).map_or(false, |md| !md.permissions().readonly())
	}

	/// This function checks if a path is readable, or at least exists.
	#[must_use]
	pub fn is_path_readable(path: PathBuf) -> bool {
		fs::metadata(path).is_ok()
	}

	/// This function is a no-op on non-MacOS systems.
	///
	/// This checks if we have full disk access available on `MacOS` or not.
	#[must_use]
	pub fn has_fda() -> bool {
		Self::is_path_rw(fda_file())
	}

	/// This function is a no-op on non-MacOS systems.
	///
	/// Once ran, it will open the "Full Disk Access" prompt.
	pub fn request_fda() -> Result<()> {
		#[cfg(target_os = "macos")]
		{
			use error::Error;
			use std::process::Command;

			Command::new("open")
				.arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
				.status()
				.map_err(|_| Error::FDAPromptError)?;
		}

		Ok(())
	}
}

#[must_use]
fn fda_file() -> PathBuf {
	home_dir()
		.unwrap_or_default()
		.join("Library/Application Support/com.apple.TCC/TCC.db")
}

#[cfg(test)]
mod tests {
	use super::DiskAccess;
	use std::fs;
	use tempfile::tempdir;

	#[test]
	#[cfg_attr(miri, ignore = "Miri can't run this test")]
	#[ignore = "CI can't run this due to lack of a GUI"]
	fn macos_open_full_disk_prompt() {
		DiskAccess::request_fda().unwrap();
	}

	#[test]
	fn has_fda() {
		assert!(DiskAccess::has_fda());
	}

	#[test]
	#[should_panic(expected = "assertion failed")]
	fn should_fail() {
		let dir = tempdir().unwrap();
		let path = dir.into_path();
		let mut perms = fs::metadata(&path).unwrap().permissions();
		perms.set_readonly(true);
		fs::set_permissions(&path, perms).unwrap();
		assert!(DiskAccess::is_path_rw(path));
	}
}
