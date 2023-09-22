use std::path::Path;

fn main() {
	println!(
		"cargo:rustc-env=FFMPEG_DIR={}",
		Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../target/Frameworks")
			.display()
	);
}
