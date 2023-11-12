//! This handles the SD code to check and request full-disk access

use std::{path::PathBuf, process::Command};

pub struct FullDiskAccess;

#[derive(thiserror::Error, Debug)]
pub enum FDAError {
	#[error("no permission to access requested directory: {0}")]
	NoPermission(PathBuf),
}

type Result<T> = std::result::Result<T, FDAError>;

impl FullDiskAccess {
	pub fn has_fda() -> Result<bool> {
		todo!()
	}

	pub fn request_fda() {
		#![cfg(target_os = "macos")]
		Command::new("open")
			.arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
			.status()
			.expect("Unable to open full disk access settings");
	}
}

#[cfg(test)]
mod tests {
	use super::FullDiskAccess;

	#[test]
	#[ignore = "CI can't run this due to lack of a GUI"]
	fn macos_open_full_disk_prompt() {
		FullDiskAccess::request_fda();
	}
}
