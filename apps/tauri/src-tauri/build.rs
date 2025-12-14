#[cfg(target_os = "macos")]
use std::process::Command;

fn main() {
	// Compile .icon to Assets.car on macOS
	#[cfg(target_os = "macos")]
	{
		let project_root = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
		let icon_source = format!("{}/../Spacedrive.icon", project_root);
		let gen_dir = format!("{}/gen", project_root);

		// Create gen directory
		std::fs::create_dir_all(&gen_dir).expect("Failed to create gen directory");

		// Check if .icon file exists
		if std::path::Path::new(&icon_source).exists() {
			println!("cargo:rerun-if-changed={}", icon_source);

			// Run actool to compile .icon to Assets.car
			let output = Command::new("xcrun")
				.args([
					"actool",
					&icon_source,
					"--compile",
					&gen_dir,
					"--output-format",
					"human-readable-text",
					"--notices",
					"--warnings",
					"--errors",
					"--output-partial-info-plist",
					&format!("{}/partial.plist", gen_dir),
					"--app-icon",
					"Spacedrive",
					"--include-all-app-icons",
					"--enable-on-demand-resources",
					"NO",
					"--development-region",
					"en",
					"--target-device",
					"mac",
					"--minimum-deployment-target",
					"11.0",
					"--platform",
					"macosx",
				])
				.output()
				.expect("Failed to execute actool");

			if !output.status.success() {
				eprintln!("actool failed: {}", String::from_utf8_lossy(&output.stderr));
			} else {
				println!("Successfully compiled Spacedrive.icon to Assets.car");
			}
		} else {
			println!("cargo:warning=Spacedrive.icon not found at {}", icon_source);
		}
	}

	// Create target-suffixed daemon binary for Tauri bundler
	// Tauri's externalBin expects binaries with target triple suffix
	let target_triple = std::env::var("TARGET").expect("TARGET not set");
	let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
	let workspace_dir = std::env::var("CARGO_WORKSPACE_DIR")
		.or_else(|_| std::env::var("CARGO_MANIFEST_DIR").map(|d| format!("{}/../../..", d)))
		.expect("Could not find workspace directory");

	let daemon_source = format!("{}/target/{}/sd-daemon", workspace_dir, profile);
	let daemon_target = format!(
		"{}/target/{}/sd-daemon-{}",
		workspace_dir, profile, target_triple
	);

	if std::path::Path::new(&daemon_source).exists() {
		// Remove existing file if it exists
		let _ = std::fs::remove_file(&daemon_target);

		// Copy the daemon binary with target architecture suffix
		if let Err(e) = std::fs::copy(&daemon_source, &daemon_target) {
			eprintln!("Warning: Failed to copy daemon: {}", e);
		}
	}

	tauri_build::build()
}
