use swift_rs::*;

pub type NSObject = *mut std::ffi::c_void;

#[allow(dead_code)]
pub enum AppThemeType {
	Light = 0 as Int,
	Dark = 1 as Int,
}

swift!(pub fn lock_app_theme(theme_type: Int));
swift!(pub fn blur_window_background(window: &NSObject));
swift!(pub fn set_titlebar_style(window: &NSObject, transparent: Bool, large: Bool));

swift!(pub fn reload_webview(webview: &NSObject));
