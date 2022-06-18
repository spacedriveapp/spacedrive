fn main() {
	#[cfg(target_os = "macos")]
	{
		use swift_rs::build::{link_swift, link_swift_package};

		link_swift();
		link_swift_package("sd-desktop-macos", "./native/macos/");
	}

	// tauri_build::build();
}
