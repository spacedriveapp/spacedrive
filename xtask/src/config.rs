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
		});

	// Build context for mustache
	let context = ConfigContext {
		native_deps,
		protoc,
		mobile_native_deps,
		android_ndk_home,
		host_tag: match system.os {
			Os::Windows => "windows-x86_64",
			Os::Linux => "linux-x86_64",
			Os::MacOS => {
				if cfg!(target_arch = "aarch64") {
					"darwin-aarch64"
				} else {
					"darwin-x86_64"
				}
			}
		},
		is_win: matches!(system.os, Os::Windows),
		is_macos: matches!(system.os, Os::MacOS),
		is_linux: matches!(system.os, Os::Linux),
		has_ios,
		has_android,
		has_lld,
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
