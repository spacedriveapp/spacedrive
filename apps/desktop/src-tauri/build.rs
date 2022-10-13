fn main() {
	#[cfg(target_os = "macos")]
	{
		use swift_rs::build::{link_swift, link_swift_package};

		link_swift("10.15"); // macOS Catalina. Earliest version that is officially supported by Apple.
		link_swift_package("sd-desktop-macos", "./native/macos/");
	}

	// #[cfg(target_os = "windows")]
	// {
	// 	use std::{env, ffi::OsStr, fs, path::PathBuf};

	// 	let cwd: PathBuf = env::current_dir().unwrap();

	// 	let vcpkg_root = env::var("VCPKG_ROOT").unwrap();
	// 	let mut ffmpeg_root: PathBuf = PathBuf::from(vcpkg_root);
	// 	ffmpeg_root.extend(&["packages", "ffmpeg_x64-windows", "bin"]);

	// 	for path in fs::read_dir(ffmpeg_root).unwrap() {
	// 		let path = path.unwrap().path().to_owned();

	// 		println!("{}", path.as_os_str().to_str().unwrap());

	// 		if let Some("dll") = path.extension().and_then(OsStr::to_str) {
	// 			let mut destination_path: PathBuf = PathBuf::from(cwd.to_str().unwrap());
	// 			destination_path.extend(&[
	// 				"apps",
	// 				"desktop",
	// 				"src-tauri",
	// 				"lib",
	// 				path.file_name().and_then(OsStr::to_str).unwrap(),
	// 			]);

	// 			println!("{}", destination_path.as_os_str().to_str().unwrap());

	// 			let _source_lock = fs::OpenOptions::new().read(true).open(path.clone());
	// 			let _destination_lock = fs::OpenOptions::new()
	// 				.create(true)
	// 				.open(destination_path.clone());

	// 			let copy_result = fs::copy(path.clone(), destination_path);

	// 			assert!(
	// 				copy_result.is_ok(),
	// 				"Could not copy required DLL: \"{}\"\n{:#?}",
	// 				path.file_name().and_then(OsStr::to_str).unwrap(),
	// 				copy_result.err()
	// 			);
	// 		} else {
	// 			break;
	// 		}
	// 	}
	// }

	tauri_build::build();
}
