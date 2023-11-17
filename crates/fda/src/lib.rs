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

use dirs::{
	audio_dir, cache_dir, config_dir, config_local_dir, data_dir, data_local_dir, desktop_dir,
	document_dir, download_dir, executable_dir, home_dir, picture_dir, preference_dir, public_dir,
	runtime_dir, state_dir, template_dir, video_dir,
};
use std::{fs, path::PathBuf};

pub mod error;

use error::Result;

pub struct FullDiskAccess(Vec<PathBuf>);

impl FullDiskAccess {
	#[cfg(target_family = "unix")]
	fn is_path_rw(path: PathBuf) -> bool {
		use std::os::unix::fs::MetadataExt;

		(fs::metadata(path)).map_or(false, |md| {
			let mode = md.mode();
			mode & 0x180 == 0x180 // rw access
		})
	}

	#[cfg(target_family = "windows")]
	pub(crate) fn is_path_rw(path: PathBuf) -> bool {
		if let Ok(md) = fs::metadata(path) {
			!md.permissions().readonly()
		} else {
			false
		}
	}

	/// [`FullDiskAccess::has_fda`] needs to be checked each time we go to access a potentially protected directory, and we need to prompt for
	/// FDA if we don't have it.
	#[must_use]
	pub fn has_fda() -> bool {
		let dirs = Self::default();
		dirs.0.into_iter().all(Self::is_path_rw)
	}

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

impl Default for FullDiskAccess {
	fn default() -> Self {
		Self(
			[
				audio_dir(),
				cache_dir(),
				config_dir(),
				config_local_dir(),
				data_dir(),
				data_local_dir(),
				desktop_dir(),
				document_dir(),
				download_dir(),
				executable_dir(),
				home_dir(),
				picture_dir(),
				preference_dir(),
				public_dir(),
				runtime_dir(),
				state_dir(),
				template_dir(),
				video_dir(),
			]
			.into_iter()
			.flatten()
			.collect(),
		)
	}
}

#[cfg(test)]
mod tests {
	use std::fs;
	use tempfile::tempdir;

	use super::FullDiskAccess;

	#[test]
	#[cfg_attr(miri, ignore = "Miri can't run this test")]
	#[ignore = "CI can't run this due to lack of a GUI"]
	fn macos_open_full_disk_prompt() {
		FullDiskAccess::request_fda().unwrap();
	}

	#[test]
	fn has_fda() {
		assert!(FullDiskAccess::has_fda());
	}

	#[test]
	#[should_panic(expected = "assertion failed")]
	fn should_fail() {
		let dir = tempdir().unwrap();
		let path = dir.into_path();
		let mut perms = fs::metadata(&path).unwrap().permissions();
		perms.set_readonly(true);
		fs::set_permissions(&path, perms).unwrap();
		assert!(FullDiskAccess::is_path_rw(path));
	}
}
