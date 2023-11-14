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
use error::{Error, Result};
use std::io::ErrorKind;
use std::{path::PathBuf, process::Command};

pub mod error;

pub struct FullDiskAccess(Vec<PathBuf>);

impl FullDiskAccess {
	async fn can_access_path(path: PathBuf) -> bool {
		match tokio::fs::read_dir(path).await {
			Ok(_) => true,
			Err(e) => !matches!(e.kind(), ErrorKind::PermissionDenied),
		}
	}

	pub async fn has_fda() -> bool {
		let dirs = Self::default();
		for dir in dirs.0 {
			if !Self::can_access_path(dir).await {
				return false;
			}
		}
		true
	}

	#[allow(clippy::missing_const_for_fn)]
	pub fn request_fda() -> Result<()> {
		#[cfg(target_os = "macos")]
		Command::new("open")
			.arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
			.status()
			.map_err(|_| Error::FDAPromptError)?;

		Ok(())
	}
}

impl Default for FullDiskAccess {
	fn default() -> Self {
		Self(
			vec![
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
	use super::FullDiskAccess;

	#[test]
	#[cfg_attr(miri, ignore = "Miri can't run this test")]
	#[ignore = "CI can't run this due to lack of a GUI"]
	fn macos_open_full_disk_prompt() {
		FullDiskAccess::request_fda().unwrap();
	}

	#[tokio::test]
	async fn has_fda() {
		FullDiskAccess::has_fda().await;
	}
}
