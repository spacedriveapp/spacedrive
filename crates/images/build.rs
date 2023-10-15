use std::path::Path;

fn main() {
	println!(
		"cargo:rustc-link-search={}",
		Path::new(env!("DEPS_PATH")).join("lib").display()
	)
}
