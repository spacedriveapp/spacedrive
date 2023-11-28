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

pub mod error;

use error::Result;

pub struct DiskAccess;

impl DiskAccess {
	/// This function is a no-op on non-MacOS systems.
	///
	/// Once ran, it will open the "Full Disk Access" prompt.
	#[allow(clippy::missing_const_for_fn)]
	pub fn request_fda() -> Result<()> {
		#[cfg(target_os = "macos")]
		{
			use crate::error::Error;
			use std::process::Command;

			Command::new("open")
				.arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
				.status()
				.map_err(|_| Error::FDAPromptError)?;
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::DiskAccess;

	#[test]
	#[cfg_attr(miri, ignore = "Miri can't run this test")]
	#[ignore = "CI can't run this due to lack of a GUI"]
	fn macos_open_full_disk_prompt() {
		DiskAccess::request_fda().unwrap();
	}
}
