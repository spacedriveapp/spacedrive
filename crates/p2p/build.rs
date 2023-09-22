use std::path::Path;

fn main() {
	let frameworks_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("./target/Frameworks");

	println!("cargo:rustc-link-search={}", frameworks_path.display());
	println!(
		"cargo:rustc-env=PROTOC={}",
		frameworks_path.join("./bin/protoc").display()
	);
}
