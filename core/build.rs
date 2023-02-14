use std::process::Command;
use swift_rs::build;

fn main() {
	let output = Command::new("git")
		.args(["rev-parse", "--short", "HEAD"])
		.output()
		.expect("error getting git hash. Does `git rev-parse --short HEAD` work for you?");
	let git_hash = String::from_utf8(output.stdout)
		.expect("Error passing output of `git rev-parse --short HEAD`");
	println!("cargo:rustc-env=GIT_HASH={git_hash}");

	build::link_swift("11.0"); // Ensure this matches the version set in your `Package.swift` file.
	build::link_swift_package("swift-lib", "../packages/macos/");
}
