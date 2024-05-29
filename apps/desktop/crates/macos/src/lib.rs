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
