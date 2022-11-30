fn main() {
	#[cfg(target_os = "macos")]
	{
		use swift_rs::build::{link_swift, link_swift_package};

		link_swift("10.15"); // macOS Catalina. Earliest version that is officially supported by Apple.
		link_swift_package("sd-desktop-macos", "./native/macos/");
	}

	#[cfg(target_env = "msvc")]
	{
		use std::{env, ffi::OsStr, fs};

		env::set_var("VCPKGRS_DYNAMIC", "1");

		#[cfg(target_arch = "x86_64")]
		env::set_var("VCPKGRS_TRIPLET", "x64-windows");

		// we need the dlls to be IN THIS SOURCE FOLDER for tauri to bundle them.

		let dlls = vcpkg::Config::new()
			.cargo_metadata(false)
			.copy_dlls(false)
			.find_package("ffmpeg")
			.map_err(|e| {
				println!("Could not find ffmpeg with vcpkg: {}", e);
			})
			.map(|lib| lib.found_dlls)
			.ok()
			.unwrap();

		for dll in dlls {
			let mut dest_path = env::current_dir().unwrap();
			dest_path.push(dll.file_name().unwrap());

			let copy_result = fs::copy(dll, &dest_path);

			assert!(
				copy_result.is_ok(),
				"Could not copy required DLL: \"{}\"\n{:#?}",
				dest_path.file_name().and_then(OsStr::to_str).unwrap(),
				copy_result.err()
			);
		}
	}

	tauri_build::build();
}
