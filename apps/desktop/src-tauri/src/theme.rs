use serde::Deserialize;
use specta::Type;

#[derive(Type, Deserialize, Clone, Copy, Debug)]
pub enum AppThemeType {
	Auto = -1,
	Light = 0,
	Dark = 1,
}

#[tauri::command(async)]
#[specta::specta]
#[allow(unused_variables)]
pub async fn lock_app_theme(theme_type: AppThemeType) {
	#[cfg(target_os = "macos")]
	unsafe {
		sd_desktop_macos::lock_app_theme(theme_type as isize);
	}
	// println!("Lock theme, type: {theme_type:?}")
}
