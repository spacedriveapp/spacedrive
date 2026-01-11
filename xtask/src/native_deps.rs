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

	println!("Downloading native dependencies from:");
	println!("   {}", url);

	let client = reqwest::blocking::Client::builder()
		.timeout(std::time::Duration::from_secs(300))
		.build()
		.context("Failed to create HTTP client")?;

	let response = client
		.get(&url)
		.send()
		.context("Failed to download native dependencies")?
		.error_for_status()
		.context("Server returned error")?;

	let total_size = response.content_length().unwrap_or(0);
	let bytes = response.bytes().context("Failed to read response")?;

	println!("Downloaded {} MB", total_size / 1_000_000);

	// Extract the archive
	extract_tar_xz(&bytes, dest_dir)?;

	Ok(())
}

/// Download iOS native dependencies
#[cfg(target_os = "macos")]
pub fn download_ios_deps(target: &str, dest_dir: &Path) -> Result<()> {
	let filename = match target {
		"aarch64-apple-ios" => "native-deps-aarch64-ios-apple.tar.xz",
		"aarch64-apple-ios-sim" => "native-deps-aarch64-iossim-apple.tar.xz",
		"x86_64-apple-ios" => "native-deps-x86_64-iossim-apple.tar.xz",
		_ => anyhow::bail!("Unknown iOS target: {}", target),
	};

	let url = format!("{}/{}", NATIVE_DEPS_URL, filename);

	println!("Downloading iOS dependencies for {}...", target);
	println!("   {}", url);

	let client = reqwest::blocking::Client::builder()
		.timeout(std::time::Duration::from_secs(300))
		.build()
		.context("Failed to create HTTP client")?;

	let response = client
		.get(&url)
		.send()
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

pub fn download_android_deps(target: &str, dest_dir: &Path) -> Result<()> {
	let filename = match target {
		"aarch64-linux-android" => "native-deps-aarch64-linux-android.tar.xz",
		"x86_64-linux-android" => "native-deps-x86_64-linux-android.tar.xz",
		_ => anyhow::bail!("Unknown Android target: {}", target),
	};

	let url = format!("{}/{}", NATIVE_DEPS_URL, filename);

	println!("Downloading Android dependencies for {}...", target);
	println!("   {}", url);

	let client = reqwest::blocking::Client::builder()
		.timeout(std::time::Duration::from_secs(300))
		.build()
		.context("Failed to create HTTP client")?;

	let response = client
		.get(&url)
		.send()
		.context("Failed to download Android dependencies")?
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
	println!("Extracting archive...");

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

		// Create Spacedrive.framework symlink for dylibs (matches v1 behavior)
		let framework = native_deps.join("Spacedrive.framework");
		if framework.exists() {
			// Sign all dylibs in the framework (required for macOS 13+)
			let libs_dir = framework.join("Libraries");
			if libs_dir.exists() {
				println!("   Signing framework libraries...");
				for entry in fs::read_dir(&libs_dir)? {
					let entry = entry?;
					let path = entry.path();
					if path.extension().and_then(|s| s.to_str()) == Some("dylib") {
						// Remove signature first
						let _ = std::process::Command::new("codesign")
							.args(&["--remove-signature", path.to_str().unwrap()])
							.output();

						// Sign with ad-hoc signature (- means ad-hoc)
						let status = std::process::Command::new("codesign")
							.args(&["-s", "-", "-f", path.to_str().unwrap()])
							.status()
							.context("Failed to run codesign")?;

						if !status.success() {
							println!("    Warning: Failed to sign {}", path.display());
						}
					}
				}
				println!("   ✓ Signed framework libraries");
			}

			let target_frameworks = root.join("target").join("Frameworks");
			fs::create_dir_all(&target_frameworks)?;

			let framework_link = target_frameworks.join("Spacedrive.framework");

			// Remove existing symlink or directory if present
			if framework_link.is_symlink() {
				let _ = fs::remove_file(&framework_link);
			} else if framework_link.exists() {
				let _ = fs::remove_dir_all(&framework_link);
			}

			unix_fs::symlink(&framework, &framework_link)
				.context("Failed to symlink Spacedrive.framework")?;

			println!("   ✓ Linked Spacedrive.framework (includes libheif)");
		}

		// Also symlink individual dylibs from lib/ to target/ for easier access
		let lib_dir = native_deps.join("lib");
		if lib_dir.exists() {
			let target = root.join("target");

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
	}

	Ok(())
}

/// Bundle libheif from Homebrew (macOS temporary solution)
///
/// This copies libheif from Homebrew to the target directory until it's included
/// in the native-deps package. On macOS, libheif is available via Homebrew and
/// we bundle it for development builds.
pub fn bundle_libheif_from_homebrew(root: &Path) -> Result<()> {
	#[cfg(target_os = "macos")]
	{
		let homebrew_libheif = Path::new("/opt/homebrew/lib/libheif.1.dylib");

		if !homebrew_libheif.exists() {
			println!(" libheif not found in Homebrew. Install with: brew install libheif");
			println!("   HEIC support will not be available.");
			return Ok(());
		}

		let target_dir = root.join("target");
		fs::create_dir_all(&target_dir)?;

		let dest = target_dir.join("libheif.1.dylib");
		fs::copy(homebrew_libheif, &dest).context("Failed to copy libheif from Homebrew")?;

		println!("   ✓ Bundled libheif from Homebrew");
	}

	Ok(())
}

/// Create symlinks for shared libraries on Linux
pub fn symlink_libs_linux(root: &Path, native_deps: &Path) -> Result<()> {
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
