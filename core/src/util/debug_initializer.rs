// ! A system for loading a default set of data on startup. This is ONLY enabled in development builds.

use crate::{
	library::{Libraries, LibraryManagerError, LibraryName},
	location::{
		delete_location, scan_location, LocationCreateArgs, LocationError, LocationManagerError,
		ScanState,
	},
	old_job::JobManagerError,
	util::AbortOnDrop,
	Node,
};

use sd_prisma::prisma::location;
use sd_utils::error::FileIOError;

use std::{
	io,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use prisma_client_rust::QueryError;
use serde::Deserialize;
use thiserror::Error;
use tokio::{
	fs::{self, metadata},
	time::sleep,
};
use tracing::{info, instrument, warn};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInitConfig {
	path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInitConfig {
	id: Uuid,
	name: LibraryName,
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

#[derive(Error, Debug)]
pub enum InitConfigError {
	#[error("error parsing the init data: {0}")]
	Json(#[from] serde_json::Error),
	#[error("job manager: {0}")]
	JobManager(#[from] JobManagerError),
	#[error("location manager: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("library manager: {0}")]
	LibraryManager(#[from] LibraryManagerError),
	#[error("query error: {0}")]
	QueryError(#[from] QueryError),
	#[error("location error: {0}")]
	LocationError(#[from] LocationError),
	#[error("failed to get current directory from environment: {0}")]
	CurrentDir(io::Error),

	#[error(transparent)]
	Processing(#[from] sd_core_heavy_lifting::Error),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

impl InitConfig {
	pub async fn load(data_dir: &Path) -> Result<Option<Self>, InitConfigError> {
		let path = std::env::current_dir()
			.map_err(InitConfigError::CurrentDir)?
			.join(std::env::var("SD_INIT_DATA").unwrap_or("sd_init.json".to_string()));

		if metadata(&path).await.is_ok() {
			let config = fs::read(&path)
				.await
				.map_err(|e| FileIOError::from((&path, e, "Failed to read init config file")))?;

			let mut config = serde_json::from_slice::<InitConfig>(&config)?;

			config.path = path;

			if config.reset_on_startup && metadata(data_dir).await.is_ok() {
				warn!("previous 'SD_DATA_DIR' was removed on startup!");
				fs::remove_dir_all(data_dir).await.map_err(|e| {
					FileIOError::from((data_dir, e, "Failed to remove data directory"))
				})?;
			}

			return Ok(Some(config));
		}

		Ok(None)
	}

	#[instrument(skip_all, fields(path = %self.path.display()), err)]
	pub async fn apply(
		self,
		library_manager: &Arc<Libraries>,
		node: &Arc<Node>,
	) -> Result<(), InitConfigError> {
		info!("Initializing app from file");

		for lib in self.libraries {
			let name = lib.name.to_string();
			let _guard = AbortOnDrop(tokio::spawn(async move {
				loop {
					info!(library_name = %name, "Initializing library from 'sd_init.json'...;");
					sleep(Duration::from_secs(1)).await;
				}
			}));

			let library = if let Some(lib) = library_manager.get_library(&lib.id).await {
				lib
			} else {
				let library = library_manager
					.create_with_uuid(lib.id, lib.name, lib.description, true, None, node)
					.await?;

				let Some(lib) = library_manager.get_library(&library.id).await else {
					warn!(
						"Debug init error: library '{}' was not found after being created!",
						library.config().await.name.as_ref()
					);
					return Ok(());
				};

				lib
			};

			if lib.reset_locations_on_startup {
				let locations = library.db.location().find_many(vec![]).exec().await?;

				for location in locations {
					warn!(location_path = ?location.path, "deleting location;");
					delete_location(node, &library, location.id).await?;
				}
			}

			for loc in lib.locations {
				if let Some(location) = library
					.db
					.location()
					.find_first(vec![location::path::equals(Some(loc.path.clone()))])
					.exec()
					.await?
				{
					warn!(location_path = ?location.path, "deleting location;");
					delete_location(node, &library, location.id).await?;
				}

				let sd_file = PathBuf::from(&loc.path).join(".spacedrive");

				if let Err(e) = fs::remove_file(sd_file).await {
					if e.kind() != io::ErrorKind::NotFound {
						warn!(?e, "failed to remove '.spacedrive' file;");
					}
				}

				if let Some(location) = (LocationCreateArgs {
					path: PathBuf::from(loc.path.clone()),
					dry_run: false,
					indexer_rules_ids: Vec::new(),
				})
				.create(node, &library)
				.await?
				{
					scan_location(node, &library, location, ScanState::Pending).await?;
				} else {
					warn!(
						location_path = ?loc.path,
						"Debug init error: location was not found after being created!",
					);
				}
			}
		}

		info!("Initialized app from file");

		Ok(())
	}
}
