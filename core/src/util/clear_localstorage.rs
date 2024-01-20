use directories::BaseDirs;
use tokio::fs;
use tracing::{info, warn};

#[cfg(target_os = "macos")]
const EXTRA_DIRS: [&str; 2] = ["Library/WebKit/Spacedrive", "Library/Caches/Spacedrive"];
#[cfg(target_os = "linux")]
const EXTRA_DIRS: [&str; 1] = [".cache/spacedrive"];

pub async fn clear_localstorage() {
	if let Some(base_dir) = BaseDirs::new() {
		// `data_local_dir` gives Local AppData on Windows, while still using `~/.local/share` and
		// `~/Library/Application Support` on Linux and MacOS respectively.
		// `com.spacedrive.desktop` is in the Local AppData directory on Windows, not the Roaming AppData
		let data_dir = base_dir.data_local_dir().join("com.spacedrive.desktop"); // maybe tie this into something static?

		fs::remove_dir_all(&data_dir)
			.await
			.map_err(|_| warn!("Unable to delete the localstorage directory"))
			.ok();

		info!("Deleted {}", data_dir.display());

		let home_dir = base_dir.home_dir();

		#[cfg(any(target_os = "macos", target_os = "linux"))]
		for path in EXTRA_DIRS {
			fs::remove_dir_all(home_dir.join(path))
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
