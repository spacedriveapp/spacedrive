use directories::BaseDirs;
use tokio::fs;
use tracing::{info, warn};

#[cfg(target_os = "macos")]
const EXTRA_DIRS: [&str; 2] = ["Library/WebKit/Spacedrive", "Library/Caches/Spacedrive"];
#[cfg(target_os = "linux")]
const EXTRA_DIRS: [&str; 1] = [".cache/spacedrive"];

pub async fn clear_localstorage() {
	if let Some(base_dirs) = BaseDirs::new() {
		let data_dir = BaseDirs::data_dir(&base_dirs).join("com.spacedrive.desktop"); // app identifier, maybe tie this into something?

		fs::remove_dir_all(data_dir)
			.await
			.map_err(|_| warn!("Unable to delete the localstorage directory"))
			.ok();

		#[cfg(any(target_os = "macos", target_os = "linux"))]
		for path in EXTRA_DIRS {
			fs::remove_dir_all(BaseDirs::home_dir(&base_dirs).join(path))
				.await
				.map_err(|_| warn!("Unable to delete {path}"))
				.ok();

			info!("Deleted {path}");
		}

		info!("Cleared localstorage fully")
	} else {
		warn!("Unable to source the base directories in order to clear localstorage")
	}
}
