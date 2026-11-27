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
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

	// Expose target triple to runtime code for daemon binary resolution
	println!("cargo:rustc-env=SD_TARGET_TRIPLE={}", target_triple);
	let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
	let workspace_dir = std::env::var("CARGO_WORKSPACE_DIR")
		.or_else(|_| std::env::var("CARGO_MANIFEST_DIR").map(|d| format!("{}/../../..", d)))
		.expect("Could not find workspace directory");

	let exe_ext = if target_triple.contains("windows") {
		".exe"
	} else {
		""
	};

	// The app ships the daemon and the CLI built beside it. target/<profile>
	// holds host builds, so copying them under another triple would bundle
	// binaries of the wrong architecture. Cross builds get their sidecars from
	// ensure-daemon-sidecar.sh.
	let sidecars = ["sd-daemon", "sd-cli"];
	let host_triple = std::env::var("HOST").expect("HOST not set");
	let source_profiles: &[&str] = if host_triple == target_triple {
		&[profile.as_str(), "release"]
	} else {
		&[]
	};
	for source_profile in source_profiles {
		for name in sidecars {
			let source = format!(
				"{}/target/{}/{}{}",
				workspace_dir, source_profile, name, exe_ext
			);
			let target = format!(
				"{}/target/{}/{}-{}{}",
				workspace_dir, source_profile, name, target_triple, exe_ext
			);

			if std::path::Path::new(&source).exists() {
				let _ = std::fs::remove_file(&target);

				if let Err(e) = std::fs::copy(&source, &target) {
					eprintln!("Warning: Failed to copy {name}: {e}");
				}
			}
		}
	}

	// tauri_build fails when an externalBin is missing, which is the state of
	// every `cargo check` or `cargo clippy` that never built the sidecars.
	// Drop the missing ones from the config for those builds so the workspace
	// still type-checks; a bundle always runs the sidecar build first, and the
	// release workflow checks both binaries are in the app.
	let missing: Vec<&str> = sidecars
		.into_iter()
		.filter(|name| {
			!std::path::Path::new(&format!(
				"{}/target/release/{}-{}{}",
				workspace_dir, name, target_triple, exe_ext
			))
			.exists()
		})
		.collect();
	if !missing.is_empty() {
		println!(
			"cargo:warning={} not found in target/release; building without them as externalBin. Run `bun run daemon:sidecar` before bundling.",
			missing.join(", ")
		);
		let present: Vec<String> = sidecars
			.into_iter()
			.filter(|name| !missing.contains(name))
			.map(|name| format!("../../../target/release/{name}"))
			.collect();
		let mut config: serde_json::Value = std::env::var("TAURI_CONFIG")
			.ok()
			.and_then(|raw| serde_json::from_str(&raw).ok())
			.unwrap_or_else(|| serde_json::json!({}));
		config["bundle"]["externalBin"] = serde_json::json!(present);
		std::env::set_var("TAURI_CONFIG", config.to_string());
	}

	// generate_context! embeds frontendDist and panics when the directory is
	// missing, which is the state of a check or clippy run that never built
	// the frontend. A debug build gets an empty one; a release build still
	// refuses so a bundle never ships without its UI.
	let frontend_dist = std::path::Path::new(&manifest_dir).join("../dist");
	if !frontend_dist.exists() {
		if profile == "release" {
			panic!(
				"{} does not exist; run `bun run build` in apps/tauri before a release build",
				frontend_dist.display()
			);
		}
		std::fs::create_dir_all(&frontend_dist).expect("create an empty apps/tauri/dist");
		println!(
			"cargo:warning=apps/tauri/dist not found; created an empty one so the desktop binary compiles without the frontend. Run `bun run build` in apps/tauri for the real UI."
		);
	}

	tauri_build::build()
}
