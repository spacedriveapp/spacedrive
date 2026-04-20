//! Cargo config generation from template

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::system::{get_best_linker, get_rust_targets, Os, SystemInfo};

#[derive(Serialize)]
struct ConfigContext {
	#[serde(rename = "nativeDeps")]
	native_deps: Option<String>,
	protoc: Option<String>,
	#[serde(rename = "mobileNativeDeps")]
	mobile_native_deps: Option<String>,
	#[serde(rename = "androidNdkHome")]
	android_ndk_home: Option<String>,
	#[serde(rename = "hostTag")]
	host_tag: &'static str,
	#[serde(rename = "isWin")]
	is_win: bool,
	#[serde(rename = "isMacOS")]
	is_macos: bool,
	#[serde(rename = "isLinux")]
	is_linux: bool,
	#[serde(rename = "hasiOS")]
	has_ios: bool,
	#[serde(rename = "hasAndroid")]
	has_android: bool,
	#[serde(rename = "hasLLD")]
	has_lld: Option<LinkerInfo>,
	#[serde(rename = "sdkPath")]
	sdk_path: String,
	#[serde(rename = "bindgenExtraClangArgs")]
	bindgen_extra_clang_args: String,
	#[serde(rename = "libclangPath")]
	libclang_path: Option<String>,
}

#[derive(Serialize)]
struct LinkerInfo {
	linker: String,
}

/// Generate .cargo/config.toml from the mustache template
pub fn generate_cargo_config(
	root: &Path,
	native_deps_dir: Option<&Path>,
	mobile_deps_dir: Option<&Path>,
) -> Result<()> {
	println!("️  Generating .cargo/config.toml...");

	let system = SystemInfo::detect()?;
	let rust_targets = get_rust_targets().unwrap_or_default();

	// Check if iOS targets are installed
	let ios_targets = [
		"aarch64-apple-ios",
		"aarch64-apple-ios-sim",
		"x86_64-apple-ios",
	];
	let has_ios = ios_targets
		.iter()
		.any(|t| rust_targets.contains(&t.to_string()));

	let android_targets = [
		"aarch64-linux-android",
		"x86_64-linux-android",
		// add more as needed
	];
	let has_android = android_targets
		.iter()
		.any(|t| rust_targets.contains(&t.to_string()));

	// Get linker info
	let has_lld = get_best_linker().map(|linker| LinkerInfo { linker });

	// Convert paths to strings and handle Windows backslashes
	let native_deps =
		native_deps_dir.map(|p| p.to_string_lossy().replace('\\', "/").to_string());

	let protoc = native_deps_dir.map(|p| {
		let protoc_name = if cfg!(target_os = "windows") {
			"protoc.exe"
		} else {
			"protoc"
		};
		p.join("bin")
			.join(protoc_name)
			.to_string_lossy()
			.replace('\\', "/")
			.to_string()
	});

	let mobile_native_deps =
		mobile_deps_dir.map(|p| p.to_string_lossy().replace('\\', "/").to_string());

	let mut has_android = has_android;
	let android_ndk_home = find_android_ndk(None).or_else(|| {
		tracing::warn!("Android NDK not found. Android targets will be skipped.");
		tracing::warn!("You can provide the location manually by setting the ANDROID_NDK_HOME environment variable.");
		tracing::warn!("Use forward slashes (/) as they are OS-independent and work on Windows, Linux, and macOS.");
		tracing::warn!("Examples:");
		tracing::warn!("  Windows: ANDROID_NDK_HOME=\"C:/Users/Username/AppData/Local/Android/Sdk/ndk/25.1.8937393\"");
		tracing::warn!("  macOS:   ANDROID_NDK_HOME=\"/Users/Username/Library/Android/sdk/ndk/25.1.8937393\"");
		tracing::warn!("  Linux:   ANDROID_NDK_HOME=\"/home/username/Android/Sdk/ndk/25.1.8937393\"");
		has_android = false;
		None
	});

	// Get macOS SDK path so bindgen can find system headers (errno.h, etc.)
	let sdk_path = if matches!(system.os, Os::MacOS) {
		std::process::Command::new("xcrun")
			.args(["--show-sdk-path"])
			.output()
			.ok()
			.filter(|o| o.status.success())
			.and_then(|o| String::from_utf8(o.stdout).ok())
			.map(|p| p.trim().to_string())
			.unwrap_or_default()
	} else {
		String::new()
	};

	// Build context for mustache
	let context = ConfigContext {
		native_deps: native_deps.clone(),
		protoc,
		mobile_native_deps,
		android_ndk_home,
		// Android NDK host tag - the prebuilt directory is always named darwin-x86_64 on macOS,
		// but the binaries are universal (fat) binaries with native ARM64 support.
		// Google kept the path name for backwards compatibility.
		host_tag: match system.os {
			Os::Windows => "windows-x86_64",
			Os::Linux => "linux-x86_64",
			Os::MacOS => "darwin-x86_64",
		},
		is_win: matches!(system.os, Os::Windows),
		is_macos: matches!(system.os, Os::MacOS),
		is_linux: matches!(system.os, Os::Linux),
		has_ios,
		has_android,
		has_lld,
		sdk_path: sdk_path.clone(),
		bindgen_extra_clang_args: match system.os {
			Os::MacOS => format!("-isysroot {}", sdk_path),
			Os::Windows => {
				let mut args = format!(
					"-I{}/include -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D_CRT_USE_BUILTIN_OFFSETOF -D_WIN32 -D_WIN64",
					native_deps.as_deref().unwrap_or("")
				);

				// Try to find Windows SDK and MSVC include paths
				// This helps bindgen find errno.h and other system headers
				if let Ok(paths) = find_windows_include_paths(None, None) {
					for path in paths {
						args.push_str(&format!(" -isystem\\\"{}\\\"", path.replace('\\', "/")));
					}
				}

				args
			}
			Os::Linux => String::new(),
		},
		libclang_path: if system.os == Os::Windows {
			find_libclang_path(None, None)
		} else {
			None
		},
	};

	// Read template
	let template_path = root.join(".cargo").join("config.toml.mustache");
	let template =
		fs::read_to_string(&template_path).context("Failed to read config.toml.mustache")?;

	// Render template
	let rendered = mustache::compile_str(&template)
		.context("Failed to compile mustache template")?
		.render_to_string(&context)
		.context("Failed to render template")?;

	// Clean up extra newlines
	let rendered = rendered
		.lines()
		.filter(|line| !line.trim().is_empty() || line.is_empty())
		.collect::<Vec<_>>()
		.join("\n");

	// Validate TOML before writing
	toml::from_str::<toml::Value>(&rendered).context("Generated config is not valid TOML")?;

	// Write output
	let output_path = root.join(".cargo").join("config.toml");
	fs::write(&output_path, rendered).context("Failed to write config.toml")?;

	println!("   ✓ Generated {}", output_path.display());

	Ok(())
}

/// Find Windows SDK and MSVC include paths
fn find_windows_include_paths(
	vs_install_path_override: Option<&str>,
	sdk_root_override: Option<&str>,
) -> Result<Vec<String>> {
	let mut paths = Vec::new();

	// 1. Find MSVC path
	let install_path = if let Some(p) = vs_install_path_override {
		p.to_string()
	} else {
		let vswhere_installer = "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe";
		let vs_path = Command::new(vswhere_installer)
			.args([
				"-latest",
				"-products",
				"*",
				"-requires",
				"Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
				"-property",
				"installationPath",
			])
			.output();

		if let Ok(output) = vs_path {
			String::from_utf8_lossy(&output.stdout).trim().to_string()
		} else {
			String::new()
		}
	};

	if !install_path.is_empty() {
			// Find latest MSVC version
			let tools_path = Path::new(&install_path).join("VC\\Tools\\MSVC");
			if let Ok(entries) = fs::read_dir(tools_path) {
				let versions = entries
					.filter_map(|e| e.ok())
					.filter(|e| e.path().is_dir())
					.filter_map(|e| e.file_name().into_string().ok());
				if let Some(latest) = get_latest_numeric_version(versions) {
					paths.push(
						Path::new(&install_path)
							.join("VC\\Tools\\MSVC")
							.join(latest)
							.join("include")
							.to_string_lossy()
							.to_string(),
					);
				}
			}
		}

	// 2. Find Windows SDK path
	let sdk_root = sdk_root_override.unwrap_or("C:\\Program Files (x86)\\Windows Kits\\10\\Include");
	if let Ok(entries) = fs::read_dir(sdk_root) {
		let versions = entries
			.filter_map(|e| e.ok())
			.filter(|e| e.path().is_dir())
			.filter_map(|e| e.file_name().into_string().ok());
		if let Some(latest) = get_latest_numeric_version(versions) {
			let latest_path = Path::new(sdk_root).join(latest);
			paths.push(latest_path.join("ucrt").to_string_lossy().to_string());
			paths.push(latest_path.join("um").to_string_lossy().to_string());
			paths.push(latest_path.join("shared").to_string_lossy().to_string());
		}
	}

	Ok(paths)
}

/// Dynamically find the path to libclang.dll
fn find_libclang_path(
	clang_bin_dir_override: Option<&str>,
	common_paths_override: Option<Vec<&str>>,
) -> Option<String> {
	// 1. Check environment variable
	if let Ok(path) = std::env::var("LIBCLANG_PATH") {
		return Some(path.replace('\\', "/"));
	}

	// 2. Try to find clang.exe in PATH and get its directory
	let clang_bin_dir = if let Some(p) = clang_bin_dir_override {
		Some(p.to_string())
	} else if let Ok(output) = Command::new("where.exe").arg("clang").output() {
		if output.status.success() {
			let path_str = String::from_utf8_lossy(&output.stdout);
			if let Some(first_line) = path_str.lines().next() {
				Path::new(first_line).parent().map(|p| p.to_string_lossy().to_string())
			} else { None }
		} else { None }
	} else { None };

	if let Some(dir) = clang_bin_dir {
		if Path::new(&dir).join("libclang.dll").exists() {
			return Some(Path::new(&dir).to_string_lossy().replace('\\', "/").to_string());
		}
	}

	// 3. Common installation paths
	let default_paths = vec![
		"C:/Program Files/LLVM/bin",
		"C:/Program Files (x86)/LLVM/bin",
	];
	let common_paths = common_paths_override.unwrap_or(default_paths);
	for path in common_paths {
		if Path::new(path).join("libclang.dll").exists() {
			return Some(path.to_string().replace('\\', "/"));
		}
	}

	None
}

/// Find Android SDK location
fn find_android_sdk(sdk_root_override: Option<&str>) -> Option<PathBuf> {
	if let Some(p) = sdk_root_override {
		return Some(PathBuf::from(p));
	}
	// 1. Environment variables
	for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
		if let Ok(path) = std::env::var(var) {
			let path = PathBuf::from(path);
			if path.exists() {
				return Some(path);
			}
		}
	}

	// 2. Default platform paths
	let mut candidates = Vec::new();

	#[cfg(target_os = "windows")]
	{
		if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
			candidates.push(PathBuf::from(local_app_data).join("Android/Sdk"));
		}
		candidates.push(PathBuf::from("C:/Android/Sdk"));
		candidates.push(PathBuf::from("C:/android-sdk"));
	}

	#[cfg(target_os = "macos")]
	{
		if let Ok(home) = std::env::var("HOME") {
			if !home.is_empty() {
				candidates.push(PathBuf::from(&home).join("Library/Android/sdk"));
			}
		}
		candidates.push(PathBuf::from("/Library/Android/sdk"));
		candidates.push(PathBuf::from("/usr/local/share/android-sdk"));
		candidates.push(PathBuf::from("/opt/homebrew/share/android-sdk"));
	}

	#[cfg(target_os = "linux")]
	{
		if let Ok(home) = std::env::var("HOME") {
			if !home.is_empty() {
				candidates.push(PathBuf::from(&home).join("Android/Sdk"));
			}
		}
		candidates.push(PathBuf::from("/usr/lib/android-sdk"));
		candidates.push(PathBuf::from("/opt/android-sdk"));
		candidates.push(PathBuf::from("/usr/local/android-sdk"));
	}

	candidates.into_iter().find(|p| p.exists())
}

/// Helper to sort and find the latest numeric version directory
fn get_latest_numeric_version<I, S>(versions: I) -> Option<String>
where
	I: Iterator<Item = S>,
	S: AsRef<str>,
{
	versions
		.map(|s| s.as_ref().to_string())
		// Filter out non-version directories
		.filter(|s| s.starts_with(|c: char| c.is_ascii_digit()))
		.max_by(|a, b| {
			let a_parts: Vec<_> = a.split('.').filter_map(|s| s.parse::<u32>().ok()).collect();
			let b_parts: Vec<_> = b.split('.').filter_map(|s| s.parse::<u32>().ok()).collect();
			a_parts.cmp(&b_parts)
		})
}

/// Find Android NDK location
fn find_android_ndk(sdk_root_override: Option<&str>) -> Option<String> {
	// 1. Direct environment variables
	for var in ["ANDROID_NDK_HOME", "ANDROID_NDK"] {
		if let Ok(path) = std::env::var(var) {
			if Path::new(&path).exists() {
				return Some(path.replace('\\', "/"));
			}
		}
	}

	// 2. Automated discovery within SDK
	if let Some(sdk_path) = find_android_sdk(sdk_root_override) {
		// Side-by-side NDK (preferred)
		let ndk_dir = sdk_path.join("ndk");
		if ndk_dir.exists() {
			if let Ok(entries) = fs::read_dir(&ndk_dir) {
				let versions = entries
					.filter_map(|e| e.ok())
					.filter(|e| e.path().is_dir())
					.filter_map(|e| e.file_name().into_string().ok());

				if let Some(latest) = get_latest_numeric_version(versions) {
					return Some(
						ndk_dir
							.join(latest)
							.to_string_lossy()
							.replace('\\', "/")
							.to_string(),
					);
				}
			}
		}

		// Legacy ndk-bundle
		let ndk_bundle = sdk_path.join("ndk-bundle");
		if ndk_bundle.exists() {
			return Some(ndk_bundle.to_string_lossy().replace('\\', "/").to_string());
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;


	#[test]
	fn test_windows_dynamic_discovery_mocked() {
		let temp = std::env::temp_dir().join("sd_test_dynamic_discovery");
		let _ = fs::remove_dir_all(&temp);
		fs::create_dir_all(&temp).unwrap();

		// Mock MSVC
		let msvc_root = temp.join("VS");
		let msvc_tools = msvc_root.join("VC").join("Tools").join("MSVC").join("14.38.33130").join("include");
		fs::create_dir_all(&msvc_tools).unwrap();

		// Mock SDK
		let sdk_root = temp.join("Windows Kits").join("10").join("Include");
		let sdk_version = sdk_root.join("10.0.22621.0");
		fs::create_dir_all(sdk_version.join("ucrt")).unwrap();
		fs::create_dir_all(sdk_version.join("um")).unwrap();
		fs::create_dir_all(sdk_version.join("shared")).unwrap();

		let paths = find_windows_include_paths(
			Some(msvc_root.to_str().unwrap()),
			Some(sdk_root.to_str().unwrap()),
		).unwrap();

		assert_eq!(paths.len(), 4);
		assert!(paths.iter().any(|p| p.contains("14.38.33130")));
		assert!(paths.iter().any(|p| p.contains("ucrt")));

		// Mock Libclang
		let llvm_root = temp.join("LLVM").join("bin");
		fs::create_dir_all(&llvm_root).unwrap();
		fs::write(llvm_root.join("libclang.dll"), "").unwrap();

		let libclang = find_libclang_path(
			None,
			Some(vec![llvm_root.to_str().unwrap()]),
		);
		assert!(libclang.is_some());

		let _ = fs::remove_dir_all(&temp);
	}

	#[test]
	fn test_android_discovery_mocked() {
		let temp = std::env::temp_dir().join("sd_test_android_discovery");
		let _ = fs::remove_dir_all(&temp);
		fs::create_dir_all(&temp).unwrap();

		// Mock NDK directory structure
		let ndk_root = temp.join("ndk");
		fs::create_dir_all(ndk_root.join("25.1.8937393")).unwrap();
		fs::create_dir_all(ndk_root.join("25.2.9519653")).unwrap();
		fs::create_dir_all(ndk_root.join(".DS_Store")).unwrap();

		let ndk = find_android_ndk(Some(temp.to_str().unwrap()));
		assert!(ndk.is_some(), "Should find NDK in mocked SDK root");
		
		let ndk_path = ndk.unwrap();
		assert!(ndk_path.contains("25.2.9519653"), "Should select the latest version: {}", ndk_path);

		let _ = fs::remove_dir_all(&temp);
	}

	#[test]
	fn test_ndk_version_sorting_logic() {
		let versions = vec![
			"21.1.6352462",
			"25.1.8937393",
			"23.2.8568313",
			"9.0.0",
			".DS_Store",
			"invalid_dir",
			"25.2.9519653", // Should beat 25.1
		];

		let latest = get_latest_numeric_version(versions.into_iter()).expect("Should find a version");
		assert_eq!(latest, "25.2.9519653", "Should pick the semver-latest version and ignore invalid dirs");
	}

	#[test]
	fn test_path_normalization_logic() {
		// Verify our OS-independent path strategy
		let windows_path = "C:\\Users\\Owner\\Android\\Sdk";
		let normalized = windows_path.replace('\\', "/");
		assert_eq!(normalized, "C:/Users/Owner/Android/Sdk");
		
		// Ensure it's a no-op on Unix-style paths
		let unix_path = "/home/user/Android/Sdk";
		assert_eq!(unix_path.replace('\\', "/"), "/home/user/Android/Sdk");
	}
}

