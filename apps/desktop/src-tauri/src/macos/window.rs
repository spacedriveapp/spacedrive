use super::native::NSObject;
use swift_rs::*;

pub_swift_fn!(lock_app_theme(theme_type: Int));
pub_swift_fn!(blur_window_background(window: NSObject));
pub_swift_fn!(set_invisible_toolbar(window: NSObject, shown: Bool));
pub_swift_fn!(set_titlebar_style(
	window: NSObject,
	transparent: Bool,
	large: Bool
));

#[allow(dead_code)]
pub enum AppThemeType {
	Light = 0 as Int,
	Dark = 1 as Int,
}
