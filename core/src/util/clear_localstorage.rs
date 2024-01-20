use directories::BaseDirs;
use tokio::fs;
use tracing::{info, warn};

pub async fn clear_localstorage() {
	if let Some(base_dirs) = BaseDirs::new() {
		let data_dir = BaseDirs::data_dir(&base_dirs).join("com.spacedrive.desktop"); // app identifier, maybe tie this into something?

		fs::remove_dir_all(data_dir)
			.await
			.map_err(|_| warn!("Unable to delete the localstorage directory"))
			.ok();

		#[cfg(target_os = "macos")]
		fs::remove_dir_all(BaseDirs::home_dir(&base_dirs).join("Library/WebKit/Spacedrive"))
			.await
			.map_err(|_| warn!("Unable to delete the WebKit localstorage directory"))
			.ok();

		#[cfg(target_os = "macos")]
		fs::remove_dir_all(BaseDirs::home_dir(&base_dirs).join("Library/Caches/Spacedrive"))
			.await
			.map_err(|_| warn!("Unable to delete the Spacedrive cache directory"))
			.ok();

		#[cfg(target_os = "linux")]
		fs::remove_dir_all(BaseDirs::home_dir(&base_dirs).join(".cache/spacedrive"))
			.await
			.map_err(|_| warn!("Unable to delete the Spacedrive cache directory"))
			.ok();

		info!("Cleared localstorage successfully")
	} else {
		warn!("Unable to source the base directories in order to clear localstorage")
	}
}
