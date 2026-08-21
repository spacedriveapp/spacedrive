//! Cargo config generation from template

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::system::{get_best_linker, get_rust_targets, Os, SystemInfo};

#[derive(Serialize)]
struct ConfigContext {
	#[serde(rename = "nativeDeps")]
	native_deps: Option<String>,
	protoc: Option<String>,
	#[serde(rename = "mobileNativeDeps")]
	mobile_native_deps: Option<String>,
	#[serde(rename = "androidNdkHome")]
	android_ndk_home: String,
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

	// A mobile target section is only useful when the libraries it points at are
	// on disk, so both the rustup target and the download have to be present.
	let ios_targets = [
		"aarch64-apple-ios",
		"aarch64-apple-ios-sim",
		"x86_64-apple-ios",
	];
	let has_ios = mobile_deps_dir.is_some()
		&& ios_targets
			.iter()
			.any(|t| rust_targets.contains(&t.to_string()));

	let android_targets = [
		"aarch64-linux-android",
		"x86_64-linux-android",
		// add more as needed
	];
	let has_android = mobile_deps_dir.is_some()
		&& android_targets
			.iter()
			.any(|t| rust_targets.contains(&t.to_string()));

	// Get linker info
	let has_lld = get_best_linker().map(|linker| LinkerInfo { linker });

	// Convert paths to strings and handle Windows backslashes
	let native_deps =
		native_deps_dir.map(|p| p.to_string_lossy().replace('\\', "\\\\").to_string());

	let protoc = native_deps_dir.map(|p| {
		let protoc_name = if cfg!(target_os = "windows") {
			"protoc.exe"
		} else {
			"protoc"
		};
		p.join("bin")
			.join(protoc_name)
			.to_string_lossy()
			.replace('\\', "\\\\")
			.to_string()
	});

	let mobile_native_deps =
		mobile_deps_dir.map(|p| p.to_string_lossy().replace('\\', "\\\\").to_string());

	let android_ndk_home = std::env::var("ANDROID_NDK")
		.or_else(|_| std::env::var("ANDROID_NDK_HOME"))
		.unwrap_or_else(|_| {
			println!("   ⚠️  Android NDK not found. Android builds will not work.");
			String::new()
		})
		.replace('\\', "\\\\");

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
		native_deps,
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
		sdk_path,
	};

	let rendered = render(root, &context)?;

	let output_path = root.join(".cargo").join("config.toml");
	fs::write(&output_path, rendered).context("Failed to write config.toml")?;

	println!("   ✓ Generated {}", output_path.display());

	Ok(())
}

/// Render the template for a context and check the result parses as TOML. The
/// template branches on which dependency bundles are present, so a combination
/// that produces a malformed section has to fail here rather than at build time.
fn render(root: &Path, context: &ConfigContext) -> Result<String> {
	let template_path = root.join(".cargo").join("config.toml.mustache");
	let template =
		fs::read_to_string(&template_path).context("Failed to read config.toml.mustache")?;

	let rendered = mustache::compile_str(&template)
		.context("Failed to compile mustache template")?
		.render_to_string(context)
		.context("Failed to render template")?;

	// Sections that render to nothing leave whitespace behind, so drop blank-but-not
	// -empty lines and collapse the runs of blanks that skipped sections leave.
	let mut lines: Vec<&str> = Vec::new();
	for line in rendered.lines() {
		let redundant_blank = !line.is_empty() || lines.last().is_none_or(|last| last.is_empty());
		if line.trim().is_empty() && redundant_blank {
			continue;
		}
		lines.push(line);
	}
	let rendered = lines.join("\n");

	toml::from_str::<toml::Value>(&rendered).context("Generated config is not valid TOML")?;

	Ok(rendered)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn context(os: Os, native_deps: bool, mobile_deps: bool) -> ConfigContext {
		ConfigContext {
			native_deps: native_deps.then(|| "/repo/apps/.deps".to_string()),
			protoc: native_deps.then(|| "/repo/apps/.deps/bin/protoc".to_string()),
			mobile_native_deps: mobile_deps.then(|| "/repo/apps/mobile/.deps".to_string()),
			android_ndk_home: "/ndk".to_string(),
			host_tag: "darwin-x86_64",
			is_win: matches!(os, Os::Windows),
			is_macos: matches!(os, Os::MacOS),
			is_linux: matches!(os, Os::Linux),
			has_ios: mobile_deps && matches!(os, Os::MacOS),
			has_android: mobile_deps,
			has_lld: Some(LinkerInfo {
				linker: "lld".to_string(),
			}),
			sdk_path: "/sdk".to_string(),
		}
	}

	fn root() -> std::path::PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.parent()
			.expect("xtask sits in the workspace root")
			.to_path_buf()
	}

	#[test]
	fn every_bundle_combination_renders_valid_toml() {
		for os in [Os::MacOS, Os::Linux, Os::Windows] {
			for native_deps in [true, false] {
				for mobile_deps in [true, false] {
					render(&root(), &context(os, native_deps, mobile_deps)).unwrap_or_else(|e| {
						panic!("{os:?} native={native_deps} mobile={mobile_deps}: {e}")
					});
				}
			}
		}
	}

	#[test]
	fn without_the_bundle_nothing_points_into_it() {
		for os in [Os::MacOS, Os::Linux, Os::Windows] {
			let rendered = render(&root(), &context(os, false, false)).unwrap();

			assert!(!rendered.contains(".deps"), "{os:?}");
			assert!(!rendered.contains("heif"), "{os:?}");
			assert!(!rendered.contains("ffmpeg"), "{os:?}");
			// A dangling search path would point the linker at the system libraries.
			assert!(!rendered.contains("\"-L\""), "{os:?}");
			// openssl-sys needs these whether or not the bundle is there.
			assert!(rendered.contains("OPENSSL_STATIC"), "{os:?}");
		}
	}

	#[test]
	fn with_the_bundle_the_aliases_enable_the_codec_features() {
		let rendered = render(&root(), &context(Os::MacOS, true, false)).unwrap();

		assert!(rendered.contains("FFMPEG_DIR"));
		assert!(rendered.contains("\"-L\", \"/repo/apps/.deps/lib\""));
		assert!(rendered.contains("[target.aarch64-apple-darwin.heif]"));
		assert!(rendered.contains("sd-daemon --features sd-core/ffmpeg,sd-core/heif"));
		assert!(rendered.contains("spacedrive --features sd-core/ffmpeg,sd-core/heif"));
	}
}
