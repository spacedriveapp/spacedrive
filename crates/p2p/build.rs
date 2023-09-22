use std::path::Path;

fn main() {
	println!(
		"cargo:rustc-env=PROTOC={}",
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../target/Frameworks/bin/protoc")
			.display()
	);
}
