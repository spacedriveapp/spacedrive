use swift_rs::build::{link_swift, link_swift_package};

fn main() {
	// HOTFIX: compile the swift code for arm64
	// std::env::set_var("CARGO_CFG_TARGET_ARCH", "arm64");

	link_swift();
	link_swift_package("sd-desktop-macos", "./native/macos/");

	tauri_build::build();
}
