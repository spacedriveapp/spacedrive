use directories::BaseDirs;
use tokio::fs;
use tracing::{info, warn};

pub async fn clear_localstorage() {
	if let Some(base_dirs) = BaseDirs::new() {
		let data_dir = BaseDirs::data_dir(&base_dirs);

		fs::remove_dir_all(data_dir.join("com.spacedrive.desktop")) // app identifier, maybe tie this into something?
			.await
			.map_err(|_| warn!("Unable to delete the localstorage directory"))
			.ok();

		info!("Cleared localstorage successfully")
	} else {
		warn!("Unable to source the base directories in order to clear localstorage")
	}
}
