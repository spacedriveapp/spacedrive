fn main() {
	#[cfg(target_os = "macos")]
	{
		use swift_rs::build::{link_swift, link_swift_package};

		link_swift("10.15"); // macOS Catalina. Earliest version that is officially supported by Apple.
		link_swift_package("sd-desktop-macos", "./native/macos/");
	}

	#[cfg(target_os = "windows")]
	{
		use std::{env, ffi::OsStr, fs, path::PathBuf};

		let vcpkg_root = env::var("VCPKG_ROOT").unwrap();
		let mut ffmpeg_root: PathBuf = PathBuf::from(vcpkg_root);
		ffmpeg_root.extend(&["packages", "ffmpeg_x64-windows", "bin"]);

		for path in fs::read_dir(ffmpeg_root).unwrap() {
			let path = path.unwrap().path().to_owned();

			println!("{}", path.as_os_str().to_str().unwrap());

			if let Some("dll") = path.extension().and_then(OsStr::to_str) {
				let copy_result = fs::copy(
					path.clone(),
					format!(".\\lib\\{:?}", path.file_name().and_then(OsStr::to_str)),
				);

				assert!(
					copy_result.is_ok(),
					"Could not copy required DLL: \"{}\"\n{:#?}",
					path.file_name().and_then(OsStr::to_str).unwrap(),
					copy_result.err()
				)
			} else {
				break;
			}
		}
	}

	// #[cfg(target_os = "windows")]
	// {
	// 	use std::{env, ffi::OsStr, fs, io, path::PathBuf};

	// 	let destination_dir = "./lib";

	// 	let vcpkg_root = env::var("VCPKG_ROOT").unwrap().as_str();

	// 	if !vcpkg_root.is_empty() {
	// 		let ffmpeg_root = format!("{}", vcpkg_root);

	// 		for path in fs::read_dir(ffmpeg_root).unwrap() {
	// 			let path = match path {
	// 				Err(e) => {
	// 					panic!("Error: {}", e);
	// 				}
	// 				Ok(p) => p,
	// 			}
	// 			.path();

	// 			if let Some("dll") = path.extension().and_then(OsStr::to_str) {
	// 				fs::copy(path, "./lib");
	// 			}
	// 		}
	// 	} else {
	// 		panic!("VCPKG_ROOT is not set! Please set a VCPKG_ROOT.")
	// 	}
	// }

	tauri_build::build();
}
