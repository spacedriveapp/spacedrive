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
}
