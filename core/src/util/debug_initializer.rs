// ! A system for loading a default set of data on startup. This is ONLY enabled in development builds.

use std::{
	path::{Path, PathBuf},
	time::Duration,
};

use crate::{
	library::LibraryConfig,
	location::{delete_location, scan_location, LocationCreateArgs},
	prisma::location,
};
use serde::Deserialize;
use tokio::{
	fs::{self, metadata},
	time::sleep,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::library::LibraryManager;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInitConfig {
	path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInitConfig {
	id: Uuid,
	name: String,
	description: Option<String>,
	#[serde(default)]
	reset_locations_on_startup: bool,
	locations: Vec<LocationInitConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitConfig {
	#[serde(default)]
	reset_on_startup: bool,
	libraries: Vec<LibraryInitConfig>,
	#[serde(skip, default)]
	path: PathBuf,
}

impl InitConfig {
	pub async fn load(data_dir: &Path) -> Option<Self> {
		let path = std::env::current_dir()
			.unwrap()
			.join(std::env::var("SD_INIT_DATA").unwrap_or("sd_init.json".to_string()));

		if metadata(&path).await.is_ok() {
			let config = fs::read_to_string(&path).await.unwrap();
			let mut config: InitConfig = serde_json::from_str(&config).unwrap();
			config.path = path;

			if config.reset_on_startup && data_dir.exists() {
				warn!("previous 'SD_DATA_DIR' was removed on startup!");
				fs::remove_dir_all(&data_dir).await.unwrap();
			}

			return Some(config);
		}

		None
	}

	pub async fn apply(self, library_manager: &LibraryManager) {
		info!("Initializing app from file: {:?}", self.path);

		for lib in self.libraries {
			let name = lib.name.clone();
			let handle = tokio::spawn(async move {
				loop {
					info!("Initializing library '{name}' from 'sd_init.json'...");
					sleep(Duration::from_secs(1)).await;
				}
			});

			let library = match library_manager.get_library(lib.id).await {
				Some(lib) => lib,
				None => {
					let library = library_manager
						.create_with_uuid(
							lib.id,
							LibraryConfig {
								name: lib.name,
								description: lib.description.unwrap_or("".to_string()),
							},
						)
						.await
						.unwrap();

					library_manager.get_library(library.uuid).await.unwrap()
				}
			};

			if lib.reset_locations_on_startup {
				let locations = library
					.db
					.location()
					.find_many(vec![])
					.exec()
					.await
					.unwrap();

				for location in locations {
					warn!("deleting location: {:?}", location.path);
					delete_location(&library, location.id).await.unwrap();
				}
			}

			for loc in lib.locations {
				if let Some(location) = library
					.db
					.location()
					.find_first(vec![location::path::equals(loc.path.clone())])
					.exec()
					.await
					.unwrap()
				{
					warn!("deleting location: {:?}", location.path);
					delete_location(&library, location.id).await.unwrap();
				}

				let sd_file = PathBuf::from(&loc.path).join(".spacedrive");
				if sd_file.exists() {
					fs::remove_file(sd_file).await.unwrap();
				}

				let location = LocationCreateArgs {
					path: loc.path.into(),
					dry_run: false,
					indexer_rules_ids: Vec::new(),
				}
				.create(&library)
				.await
				.unwrap()
				.unwrap();

				scan_location(&library, location).await.unwrap();
			}

			handle.abort();
		}

		info!("Initialized app from file: {:?}", self.path);
	}
}
