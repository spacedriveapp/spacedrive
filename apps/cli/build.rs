use std::process::Command;

/// Embeds the git commit the CLI was built from as `SD_GIT_SHA`.
///
/// The nightly channel moves one `nightly` tag along a branch, so the package
/// version never changes between nightly builds and the updater compares
/// commits instead. CI sets `SD_GIT_SHA` explicitly because a shallow or
/// detached checkout may still report the right commit through `git`, but the
/// workflow's own sha is the one the release is tagged with.
fn main() {
	println!("cargo:rerun-if-env-changed=SD_GIT_SHA");
	println!("cargo:rerun-if-changed=../../.git/HEAD");

	let sha = std::env::var("SD_GIT_SHA")
		.ok()
		.filter(|s| !s.trim().is_empty())
		.or_else(|| {
			Command::new("git")
				.args(["rev-parse", "HEAD"])
				.output()
				.ok()
				.filter(|o| o.status.success())
				.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
		})
		.unwrap_or_else(|| "unknown".to_string());

	println!("cargo:rustc-env=SD_GIT_SHA={}", sha);

	// The desktop app builds this binary with sd-core's ai-local feature,
	// whose ONNX Runtime archive needs the clang runtime helper that rustc's
	// -nodefaultlibs link drops. sd-core's build script adds the flag only to
	// its own binaries, and this script cannot see sd-core's features, so every
	// macOS build links the runtime; it adds nothing when unused.
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
		println!("cargo:rustc-link-arg=-fapple-link-rtlib");
	}
}
