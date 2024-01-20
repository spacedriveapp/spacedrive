use directories::BaseDirs;
use tokio::fs;
use tracing::{info, warn};

#[cfg(target_os = "linux")]
const EXTRA_DIRS: [&str; 1] = [".cache/spacedrive"];
#[cfg(target_os = "macos")]
const EXTRA_DIRS: [&str; 2] = ["Library/WebKit/Spacedrive", "Library/Caches/Spacedrive"];

pub async fn clear_localstorage() {
	if let Some(base_dir) = BaseDirs::new() {
		// this equates to `~/.local/share` on Linux, ~/Library/Application Support` on MacOS
		// and `~/AppData/Local` on Windows. you can find `localStorage` and other cached files here, as
		// well as in a few other dedicated paths on Linux and MacOS (which are cleared below)
		let data_dir = base_dir.data_local_dir().join("com.spacedrive.desktop"); // maybe tie this into something static?

		fs::remove_dir_all(&data_dir)
			.await
			.map_err(|_| warn!("Unable to delete the `localStorage` primary directory."))
			.ok();

		info!("Deleted {}", data_dir.display());

		let home_dir = base_dir.home_dir();

		#[cfg(any(target_os = "linux", target_os = "macos"))]
		for path in EXTRA_DIRS {
			fs::remove_dir_all(home_dir.join(path))
				.await
				.map_err(|_| warn!("Unable to delete a `localStorage` cache: {path}"))
				.ok();

			info!("Deleted {path}");
		}

		info!("Successfully wiped `localStorage` and related caches.")
	} else {
		warn!("Unable to source `BaseDirs` in order to clear `localStorage`.")
	}
}
