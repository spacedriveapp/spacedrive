#![cfg(target_os = "macos")]

use swift_rs::{swift, Bool, Int, SRData, SRObjectArray, SRString};

pub type NSObject = *mut std::ffi::c_void;

pub enum AppThemeType {
	Light = 0 as Int,
	Dark = 1 as Int,
}

swift!(pub fn disable_app_nap(reason: &SRString) -> Bool);
swift!(pub fn enable_app_nap() -> Bool);
swift!(pub fn lock_app_theme(theme_type: Int));
swift!(pub fn set_titlebar_style(window: &NSObject, is_fullscreen: Bool));
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

swift!(pub fn begin_native_drag(
	window: &NSObject,
	items: &SRString,
	overlay_window: &NSObject,
	session_id: &SRString
) -> Bool);

swift!(pub fn end_native_drag(session_id: &SRString));
swift!(pub fn update_drag_overlay_position(session_id: &SRString, x: f64, y: f64));

// Callback from Swift when drag session ends
static mut DRAG_ENDED_CALLBACK: Option<Box<dyn Fn(&str, bool) + Send + Sync>> = None;

pub fn set_drag_ended_callback<F>(callback: F)
where
	F: Fn(&str, bool) + Send + Sync + 'static,
{
	unsafe {
		DRAG_ENDED_CALLBACK = Some(Box::new(callback));
	}
}

#[no_mangle]
pub extern "C" fn rust_drag_ended_callback(session_id: *const std::ffi::c_char, was_dropped: Bool) {
	let session_id_str = unsafe {
		std::ffi::CStr::from_ptr(session_id)
			.to_string_lossy()
			.into_owned()
	};

	unsafe {
		let callback_ptr = &raw const DRAG_ENDED_CALLBACK;
		if let Some(callback) = (*callback_ptr).as_ref() {
			callback(&session_id_str, was_dropped);
		}
	}
}
