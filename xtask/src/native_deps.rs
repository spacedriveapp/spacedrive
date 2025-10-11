#![allow(warnings)]

//! Native dependencies download and extraction

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use xz2::read::XzDecoder;

const NATIVE_DEPS_URL: &str =
	"https://github.com/spacedriveapp/native-deps/releases/latest/download";

/// Download native dependencies for the current platform
pub fn download_native_deps(filename: &str, dest_dir: &Path) -> Result<()> {
	let url = format!("{}/{}", NATIVE_DEPS_URL, filename);

	println!("📦 Downloading native dependencies from:");
	println!("   {}", url);

	let response = reqwest::blocking::get(&url)
		.context("Failed to download native dependencies")?
		.error_for_status()
		.context("Server returned error")?;

	let total_size = response.content_length().unwrap_or(0);
	let bytes = response.bytes().context("Failed to read response")?;

	println!("📦 Downloaded {} MB", total_size / 1_000_000);

	// Extract the archive
	extract_tar_xz(&bytes, dest_dir)?;

	Ok(())
}

/// Download iOS native dependencies
pub fn download_ios_deps(target: &str, dest_dir: &Path) -> Result<()> {
	let filename = match target {
		"aarch64-apple-ios" => "native-deps-aarch64-ios-apple.tar.xz",
		"aarch64-apple-ios-sim" => "native-deps-aarch64-iossim-apple.tar.xz",
		"x86_64-apple-ios" => "native-deps-x86_64-iossim-apple.tar.xz",
		_ => anyhow::bail!("Unknown iOS target: {}", target),
	};

	let url = format!("{}/{}", NATIVE_DEPS_URL, filename);

	println!("📦 Downloading iOS dependencies for {}...", target);
	println!("   {}", url);

	let response = reqwest::blocking::get(&url)
		.context("Failed to download iOS dependencies")?
		.error_for_status()
		.context("Server returned error")?;

	let bytes = response.bytes().context("Failed to read response")?;

	// Create target-specific directory
	let target_dir = dest_dir.join(target);
	fs::create_dir_all(&target_dir)?;

	// Extract the archive
	extract_tar_xz(&bytes, &target_dir)?;

	println!("   ✓ Extracted to {}", target_dir.display());

	Ok(())
}

/// Extract a .tar.xz archive
fn extract_tar_xz(data: &[u8], dest: &Path) -> Result<()> {
	println!("📦 Extracting archive...");

	// Decompress XZ
	let xz_decoder = XzDecoder::new(data);

	// Extract tar
	let mut archive = tar::Archive::new(xz_decoder);
	archive.unpack(dest).context("Failed to extract archive")?;

	println!("   ✓ Extracted successfully");
	Ok(())
}

/// Create symlinks for shared libraries on macOS
pub fn symlink_libs_macos(root: &Path, native_deps: &Path) -> Result<()> {
	#[cfg(target_os = "macos")]
	{
		use std::os::unix::fs as unix_fs;

		let lib_dir = native_deps.join("lib");
		if !lib_dir.exists() {
			return Ok(()); // No libs to symlink
		}

		// Create symlink in root for FFmpeg
		let target = root.join("target");
		fs::create_dir_all(&target)?;

		for entry in fs::read_dir(&lib_dir)? {
			let entry = entry?;
			let filename = entry.file_name();
			let filename_str = filename.to_string_lossy();

			// Only symlink dylibs
			if filename_str.ends_with(".dylib") {
				let src = entry.path();
				let dst = target.join(&filename);

				// Remove existing symlink if present
				let _ = fs::remove_file(&dst);

				unix_fs::symlink(&src, &dst)
					.with_context(|| format!("Failed to symlink {}", filename_str))?;
			}
		}
	}

	Ok(())
}

/// Create symlinks for shared libraries on Linux
pub fn symlink_libs_linux(_root: &Path, _native_deps: &Path) -> Result<()> {
	#[cfg(target_os = "linux")]
	{
		use std::os::unix::fs as unix_fs;

		let lib_dir = native_deps.join("lib");
		if !lib_dir.exists() {
			return Ok(()); // No libs to symlink
		}

		// Create lib directory in root
		let target_lib = root.join("target").join("lib").join("spacedrive");
		fs::create_dir_all(&target_lib)?;

		for entry in fs::read_dir(&lib_dir)? {
			let entry = entry?;
			let filename = entry.file_name();
			let filename_str = filename.to_string_lossy();

			// Only symlink .so files
			if filename_str.contains(".so") {
				let src = entry.path();
				let dst = target_lib.join(&filename);

				// Remove existing symlink if present
				let _ = fs::remove_file(&dst);

				unix_fs::symlink(&src, &dst)
					.with_context(|| format!("Failed to symlink {}", filename_str))?;
			}
		}
	}

	Ok(())
}
