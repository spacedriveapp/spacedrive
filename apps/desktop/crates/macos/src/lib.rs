use swift_rs::{swift, Bool, Int, SRData, SRObjectArray, SRString};

pub type NSObject = *mut std::ffi::c_void;

pub enum AppThemeType {
	Light = 0 as Int,
	Dark = 1 as Int,
}

swift!(pub fn lock_app_theme(theme_type: Int));
swift!(pub fn blur_window_background(window: &NSObject));
swift!(pub fn set_titlebar_style(window: &NSObject, is_fullscreen: Bool));
// swift!(pub fn setup_disk_watcher(window: &NSObject, transparent: Bool, large: Bool));
// swift!(pub fn disk_event_callback(mounted: Bool, path: &SRString));
swift!(pub fn reload_webview(webview: &NSObject));

#[repr(C)]
pub struct OpenWithApplication {
	pub name: SRString,
	pub id: SRString,
	pub url: SRString,
	pub icon: SRData,
}

swift!(pub fn get_open_with_applications(url: &SRString) -> SRObjectArray<OpenWithApplication>);
swift!(pub(crate) fn open_file_path_with(file_url: &SRString, with_url: &SRString));

pub fn open_file_paths_with(file_urls: &[String], with_url: &str) {
	let file_url = file_urls.join("\0");
	unsafe { open_file_path_with(&file_url.as_str().into(), &with_url.into()) }
}

// main!(|_| {
// 	unsafe { setup_disk_watcher() };
// 	print!("Waiting for disk events... ");
// 	Ok(())
// });

// #[no_mangle]
// pub extern "C" fn disk_event_callback(mounted: Bool, path: *const SRString) {
// 	let mounted_str = if mounted { "mounted" } else { "unmounted" };

// 	// Convert the raw pointer to a reference
// 	let path_ref = unsafe { &*path };
// 	let path_str = path_ref.to_string(); // Assuming SRString has a to_string method

// 	println!("Disk at path {} was {}", path_str, mounted_str);
// }
