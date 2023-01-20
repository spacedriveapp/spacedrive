use super::native::NSObject;
use swift_rs::*;

#[allow(unused)]
extern "C" {
	pub fn lock_app_theme(theme_type: Int);
	pub fn blur_window_background(window: &NSObject);
	pub fn set_titlebar_style(window: &NSObject, transparent: Bool, large: Bool);
}

#[allow(dead_code)]
pub enum AppThemeType {
	Light = 0 as Int,
	Dark = 1 as Int,
}
