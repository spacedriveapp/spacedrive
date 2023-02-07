use std::process::Command;

fn main() {
	let output = Command::new("git")
		.args(["rev-parse", "--short", "HEAD"])
		.output()
		.expect("error getting git hash. Does `git rev-parse --short HEAD` work for you?");
	let git_hash = String::from_utf8(output.stdout)
		.expect("Error passing output of `git rev-parse --short HEAD`");
	println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
