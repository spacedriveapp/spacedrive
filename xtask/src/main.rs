//! Build automation tasks for Spacedrive
//!
//! This crate provides build tasks using the xtask pattern - a Rust-native
//! approach to build automation. No bash scripts, no JavaScript, no external tools.
//!
//! ## Usage
//!
//! ```bash
//! cargo xtask setup        # Setup dev environment (replaces pnpm prep)
//! cargo xtask build-ios    # Build iOS XCFramework
//! cargo ios                # Convenient alias for build-ios
//! ```
//!
//! ## About xtask
//!
//! The xtask pattern is the idiomatic Rust way to handle build automation.
//! It's just a regular Rust binary in your workspace that you invoke via
//! `cargo xtask <command>`. This approach is used by major projects like
//! rust-analyzer, tokio, and many others.
//!
//! Benefits:
//! - Pure Rust - no shell scripts or JavaScript to maintain
//! - Type-safe and easy to debug
//! - Cross-platform by default
//! - No external tools required (except cargo/rustup)

mod config;
mod native_deps;
mod system;

use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

fn main() -> Result<()> {
	let args: Vec<String> = std::env::args().collect();

	if args.len() < 2 {
		eprintln!("Usage: cargo xtask <command>");
		eprintln!();
		eprintln!("Commands:");
		eprintln!(
			"  setup        Setup development environment (downloads deps, generates config)"
		);
		eprintln!("  build-ios    Build sd-ios-core XCFramework for iOS devices and simulator");
		eprintln!("  build-mobile Build sd-mobile-core for React Native iOS/Android");
		eprintln!();
		eprintln!("Examples:");
		eprintln!("  cargo xtask setup          # First time setup");
		eprintln!("  cargo xtask build-ios      # Build iOS framework");
		eprintln!("  cargo xtask build-mobile   # Build mobile core for React Native");
		eprintln!("  cargo ios                  # Convenient alias for build-ios");
		std::process::exit(1);
	}

	match args[1].as_str() {
		"setup" => setup()?,
		"build-ios" => build_ios()?,
		"build-mobile" => build_mobile()?,
		_ => {
			eprintln!("Unknown command: {}", args[1]);
			eprintln!("Run 'cargo xtask' for usage information.");
			std::process::exit(1);
		}
	}

	Ok(())
}

/// Setup development environment
///
/// This replaces the old `pnpm prep` workflow with a pure Rust implementation.
/// It downloads native dependencies and generates the cargo config.
fn setup() -> Result<()> {
	println!("Setting up Spacedrive development environment...");
	println!();

	let project_root = std::env::current_dir()?;

	// Detect system
	let system = system::SystemInfo::detect()?;
	println!("Detected platform: {:?} {:?}", system.os, system.arch);

	// Check for required tools
	println!("Checking for required tools...");
	if !system::has_linker("cargo") {
		anyhow::bail!("cargo not found. Please install Rust from https://rustup.rs");
	}
	if !system::has_linker("rustc") {
		anyhow::bail!("rustc not found. Please install Rust from https://rustup.rs");
	}
	println!("   ✓ Rust toolchain found");

	// Setup native dependencies directory
	let native_deps_dir = project_root.join("apps").join(".deps");
	println!();
	println!("Setting up native dependencies...");

	// Clean and create deps directory
	if native_deps_dir.exists() {
		fs::remove_dir_all(&native_deps_dir).context("Failed to clean native deps directory")?;
	}
	fs::create_dir_all(&native_deps_dir).context("Failed to create native deps directory")?;

	// Download desktop native dependencies
	let filename = system.native_deps_filename();
	native_deps::download_native_deps(&filename, &native_deps_dir)?;

	// Create symlinks for shared libraries
	#[cfg(target_os = "macos")]
	{
		println!();
		println!("Creating symlinks for shared libraries...");
		native_deps::symlink_libs_macos(&project_root, &native_deps_dir)?;
	}

	#[cfg(target_os = "linux")]
	{
		println!();
		println!("Creating symlinks for shared libraries...");
		native_deps::symlink_libs_linux(&project_root, &native_deps_dir)?;
		println!("   ✓ Symlinks created");
	}

	// Download iOS dependencies if on macOS and iOS targets are installed
	#[cfg(target_os = "macos")]
	{
		let rust_targets = system::get_rust_targets().unwrap_or_default();
		let ios_targets = [
			"aarch64-apple-ios",
			"aarch64-apple-ios-sim",
			"x86_64-apple-ios",
		];

		let has_ios_targets: Vec<_> = ios_targets
			.iter()
			.filter(|t| rust_targets.contains(&t.to_string()))
			.collect();

		if !has_ios_targets.is_empty() {
			println!();
			println!("iOS targets detected, downloading iOS dependencies...");

			let mobile_deps_dir = project_root.join("apps").join("mobile").join(".deps");
			fs::create_dir_all(&mobile_deps_dir)?;

			for target in has_ios_targets {
				native_deps::download_ios_deps(target, &mobile_deps_dir)?;
			}
		} else {
			println!();
			println!("️  No iOS targets installed. Skipping iOS dependencies.");
			println!("   To add iOS support, run:");
			println!(
				"   rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios"
			);
		}
	}
	{
		let rust_targets = system::get_rust_targets().unwrap_or_default();
		let android_targets = ["aarch64-linux-android", "x86_64-linux-android"];

		let has_android_targets: Vec<_> = android_targets
			.iter()
			.filter(|t| rust_targets.contains(&t.to_string()))
			.collect();

		if !has_android_targets.is_empty() {
			println!();
			println!("Android targets detected, downloading Android dependencies...");

			let mobile_deps_dir = project_root.join("apps").join("mobile").join(".deps");
			fs::create_dir_all(&mobile_deps_dir)?;

			for target in has_android_targets {
				native_deps::download_android_deps(target, &mobile_deps_dir)?;
			}
		} else {
			println!();
			println!("️  No Android targets installed. Skipping Android dependencies.");
			println!("   To add Android support, run:");
			println!("   rustup target add aarch64-linux-android x86_64-linux-android");
		}
	}

	// Generate cargo config
	println!();
	let mobile_deps_dir = project_root.join("apps").join("mobile").join(".deps");
	let mobile_deps = if mobile_deps_dir.exists() {
		Some(mobile_deps_dir.as_path())
	} else {
		None
	};

	config::generate_cargo_config(&project_root, Some(&native_deps_dir), mobile_deps)?;

	println!();
	println!("Setup complete!");
	println!();
	println!("Next steps:");
	println!("   • cargo build              - Build the CLI");
	println!("   • cargo xtask build-ios    - Build iOS framework (macOS only)");
	println!("   • cargo ios                - Shortcut for build-ios");
	println!();

	Ok(())
}

/// Build sd-ios-core for iOS devices and simulator, creating an XCFramework
///
/// This task:
/// 1. Builds for aarch64-apple-ios (physical devices)
/// 2. Builds for aarch64-apple-ios-sim (M1/M2 simulator)
/// 3. Builds for x86_64-apple-ios (Intel simulator)
/// 4. Creates the XCFramework directory structure
/// 5. Copies the static libraries to the correct locations
/// 6. Generates all required Info.plist files
///
/// The resulting XCFramework is placed in `apps/ios/sd-ios-core/` where
/// Xcode can automatically use it.
fn build_ios() -> Result<()> {
	println!("Building Spacedrive v2 Core XCFramework for iOS...");
	println!();

	let project_root = std::env::current_dir()?;
	let ios_core_dir = project_root.join("apps/ios/sd-ios-core");
	let framework_name = "sd_ios_core";

	// Target triple and corresponding XCFramework architecture directory
	let targets = [
		("aarch64-apple-ios", "ios-arm64", false),
		("aarch64-apple-ios-sim", "ios-arm64-simulator", true),
		("x86_64-apple-ios", "ios-x86_64-simulator", true),
	];

	// Build for each target
	for (target, arch, is_sim) in &targets {
		let platform = if *is_sim { "Simulator" } else { "Device" };
		println!("Building for iOS {} ({})...", platform, arch);

		let status = Command::new("cargo")
			.args(["build", "--release", "--target", target])
			.current_dir(&ios_core_dir)
			.env("IPHONEOS_DEPLOYMENT_TARGET", "12.0")
			.status()
			.context(format!("Failed to build for {}", target))?;

		if !status.success() {
			anyhow::bail!("Build failed for target: {}", target);
		}
		println!("{} build complete", platform);
	}

	println!();
	println!("Creating XCFramework...");

	let build_dir = ios_core_dir.join("build");
	std::fs::create_dir_all(&build_dir).context("Failed to create build directory")?;

	// Create framework structure for device (ARM64)
	let device_framework_dir = build_dir.join(format!("{}-device.framework", framework_name));
	std::fs::create_dir_all(&device_framework_dir)
		.context("Failed to create device framework directory")?;

	std::fs::copy(
		ios_core_dir.join(format!(
			"target/aarch64-apple-ios/release/lib{}.a",
			framework_name
		)),
		device_framework_dir.join(framework_name),
	)
	.context("Failed to copy device library")?;

	// Create Info.plist for device framework
	let device_plist = create_framework_info_plist(framework_name, "iPhoneOS");
	std::fs::write(device_framework_dir.join("Info.plist"), device_plist)
		.context("Failed to write device Info.plist")?;

	// Create framework structure for simulator (universal - ARM64 + x86_64)
	let sim_framework_dir = build_dir.join(format!("{}-simulator.framework", framework_name));
	std::fs::create_dir_all(&sim_framework_dir)
		.context("Failed to create simulator framework directory")?;

	// Create universal simulator library using lipo
	println!("Creating universal simulator library...");
	let status = Command::new("lipo")
		.args([
			"-create",
			ios_core_dir
				.join(format!(
					"target/aarch64-apple-ios-sim/release/lib{}.a",
					framework_name
				))
				.to_str()
				.unwrap(),
			ios_core_dir
				.join(format!(
					"target/x86_64-apple-ios/release/lib{}.a",
					framework_name
				))
				.to_str()
				.unwrap(),
			"-output",
			sim_framework_dir.join(framework_name).to_str().unwrap(),
		])
		.status()
		.context("Failed to create universal simulator library")?;

	if !status.success() {
		anyhow::bail!("lipo failed to create universal simulator library");
	}

	// Create Info.plist for simulator framework
	let sim_plist = create_framework_info_plist(framework_name, "iPhoneSimulator");
	std::fs::write(sim_framework_dir.join("Info.plist"), sim_plist)
		.context("Failed to write simulator Info.plist")?;

	// Update existing XCFramework (which Xcode is already using)
	let xcframework_path = ios_core_dir.join(format!("{}.xcframework", framework_name));

	println!("Updating XCFramework at: {}", xcframework_path.display());

	// Update device framework
	let device_target = xcframework_path.join("ios-arm64");
	std::fs::copy(
		device_framework_dir.join(framework_name),
		device_target.join(format!("lib{}.a", framework_name)),
	)
	.context("Failed to update device library in XCFramework")?;

	// Update simulator framework
	let sim_target = xcframework_path.join("ios-arm64-simulator");
	std::fs::copy(
		sim_framework_dir.join(framework_name),
		sim_target.join(format!("lib{}.a", framework_name)),
	)
	.context("Failed to update simulator library in XCFramework")?;

	// Clean up build directory
	std::fs::remove_dir_all(&build_dir).context("Failed to clean up build directory")?;

	println!();
	println!("XCFramework updated successfully!");
	println!("XCFramework location: {}", xcframework_path.display());
	println!("Xcode will automatically use the updated framework");
	println!();
	println!("iOS Core build complete! Ready to test.");

	Ok(())
}

/// Build sd-mobile-core for React Native (iOS and Android)
///
/// This task builds the mobile core for use with Expo/React Native.
/// For iOS: Builds static libraries for device and simulator targets
/// For Android: Builds shared libraries for arm64-v8a (and optionally x86_64)
fn build_mobile() -> Result<()> {
	println!("Building Spacedrive Mobile Core for React Native...");
	println!();

	let project_root = std::env::current_dir()?;
	let mobile_core_dir = project_root.join("apps/mobile/modules/sd-mobile-core/core");

	if !mobile_core_dir.exists() {
		anyhow::bail!(
			"Mobile core directory not found: {}",
			mobile_core_dir.display()
		);
	}

	// Check which iOS targets are installed (macOS only)
	#[cfg(target_os = "macos")]
	{
		let rust_targets = system::get_rust_targets().unwrap_or_default();
		let ios_targets = [
			("aarch64-apple-ios", "Device", false),
			("aarch64-apple-ios-sim", "Simulator (arm64)", true),
		];

		let available_ios_targets: Vec<_> = ios_targets
			.iter()
			.filter(|(target, _, _)| rust_targets.contains(&target.to_string()))
			.collect();

		if !available_ios_targets.is_empty() {
			println!("Building for iOS targets...");
			for (target, name, _is_sim) in available_ios_targets {
				println!("  Building for iOS {} ({})...", name, target);

				let status = Command::new("cargo")
					.args(["build", "--release", "--target", target])
					.current_dir(&mobile_core_dir)
					.env("IPHONEOS_DEPLOYMENT_TARGET", "18.0")
					.status()
					.context(format!("Failed to build for {}", target))?;

				if !status.success() {
					anyhow::bail!("Build failed for target: {}", target);
				}

				println!("  ✓ {} build complete", name);
			}
		} else {
			println!("No iOS targets installed. Skipping iOS builds.");
			println!("  To add iOS support, run:");
			println!("  rustup target add aarch64-apple-ios aarch64-apple-ios-sim");
		}
	}

	// Check which Android targets are installed
	let rust_targets = system::get_rust_targets().unwrap_or_default();
	let android_targets = [
		("aarch64-linux-android", "Device", false),
		("x86_64-linux-android", "Android Emulator", true),
	];

	let available_android_targets: Vec<_> = android_targets
		.iter()
		.filter(|(target, _, _)| rust_targets.contains(&target.to_string()))
		.collect();

	if !available_android_targets.is_empty() {
		println!("Building for Android targets...");
		for (target, name, _is_emulator) in available_android_targets {
			println!("  Building for Android {} ({})...", name, target);

			let status = Command::new("cargo")
				.args(["build", "--release", "--target", target])
				.current_dir(&mobile_core_dir)
				.status()
				.context(format!("Failed to build for {}", target))?;

			if !status.success() {
				anyhow::bail!("Build failed for target: {}", target);
			}

			println!("  ✓ {} build complete", name);
		}
	} else {
		println!("No Android targets installed. Skipping Android builds.");
		println!("  To add Android support, run:");
		println!("  rustup target add aarch64-linux-android x86_64-linux-android");
	}

	// Copy built libraries to the iOS module directory
	let ios_module_dir = project_root.join("apps/mobile/modules/sd-mobile-core/ios");
	let target_dir = mobile_core_dir.join("target");

	// Create libs directory structure
	let libs_dir = ios_module_dir.join("libs");
	fs::create_dir_all(libs_dir.join("device"))?;
	fs::create_dir_all(libs_dir.join("simulator"))?;

	// Copy device library
	let device_lib = target_dir.join("aarch64-apple-ios/release/libsd_mobile_core.a");
	if device_lib.exists() {
		fs::copy(&device_lib, libs_dir.join("device/libsd_mobile_core.a"))?;
		println!("  ✓ Copied device library");
	}

	// Copy simulator library
	let sim_lib = target_dir.join("aarch64-apple-ios-sim/release/libsd_mobile_core.a");
	if sim_lib.exists() {
		fs::copy(&sim_lib, libs_dir.join("simulator/libsd_mobile_core.a"))?;
		println!("  ✓ Copied simulator library");
	}

	println!();
	println!("Mobile core build complete!");
	println!();
	println!("Libraries are in: {}", libs_dir.display());
	println!();
	println!("Next steps:");
	println!("  cd apps/mobile && bun run prebuild:clean");
	println!();

	Ok(())
}

/// Generate an Info.plist file for a framework
///
/// Creates the metadata file that describes the framework, including
/// bundle identifier, version, and supported platform.
fn create_framework_info_plist(framework_name: &str, platform: &str) -> String {
	format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive.core</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleSupportedPlatforms</key>
    <array>
        <string>{}</string>
    </array>
    <key>MinimumOSVersion</key>
    <string>12.0</string>
</dict>
</plist>
"#,
		framework_name, framework_name, platform
	)
}
