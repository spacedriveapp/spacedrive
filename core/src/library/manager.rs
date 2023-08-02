use crate::{
	invalidate_query,
	location::{indexer, LocationManagerError},
	node::{NodeConfig, Platform},
	object::{
		preview::get_thumbnails_directory,
		tag,
		thumbnail_remover::{ThumbnailRemoverActor, ThumbnailRemoverActorProxy},
	},
	prisma::location,
	util::{
		db::{self, MissingFieldError},
		error::{FileIOError, NonUtf8PathError},
		migrator::{Migrate, MigratorError},
		MaybeUndefined,
	},
	NodeServices,
};

use std::{
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use chrono::Utc;
use sd_p2p::spacetunnel::{Identity, IdentityErr};
use sd_prisma::prisma::instance;
use thiserror::Error;
use tokio::{fs, io, sync::RwLock, try_join};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{Library, LibraryConfig, LibraryConfigWrapped, LibraryName};

/// LibraryManager is a singleton that manages all libraries for a node.
pub struct LibraryManager {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<Vec<Arc<Library>>>,
	/// holds the context for the node which this library manager is running on.
	pub node: Arc<NodeServices>,
	/// An actor that removes stale thumbnails from the file system
	thumbnail_remover: ThumbnailRemoverActor,
}

#[derive(Error, Debug)]
pub enum LibraryManagerError {
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error("error serializing or deserializing the JSON in the config file: {0}")]
	Json(#[from] serde_json::Error),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("library not found error")]
	LibraryNotFound,
	#[error("error migrating the config file: {0}")]
	Migration(String),
	#[error("failed to parse uuid: {0}")]
	Uuid(#[from] uuid::Error),
	#[error("failed to run indexer rules seeder: {0}")]
	IndexerRulesSeeder(#[from] indexer::rules::seed::SeederError),
	// #[error("failed to initialise the key manager: {0}")]
	// KeyManager(#[from] sd_crypto::Error),
	#[error("failed to run library migrations: {0}")]
	MigratorError(#[from] MigratorError),
	#[error("error migrating the library: {0}")]
	MigrationError(#[from] db::MigrationError),
	#[error("invalid library configuration: {0}")]
	InvalidConfig(String),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error("failed to watch locations: {0}")]
	LocationWatcher(#[from] LocationManagerError),
	#[error("failed to parse library p2p identity: {0}")]
	Identity(#[from] IdentityErr),
	#[error("current instance with id '{0}' was not found in the database")]
	CurrentInstanceNotFound(String),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),
}

impl From<LibraryManagerError> for rspc::Error {
	fn from(error: LibraryManagerError) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			error.to_string(),
			error,
		)
	}
}

impl LibraryManager {
	pub(crate) async fn new(
		libraries_dir: PathBuf,
		node: Arc<NodeServices>,
	) -> Result<Arc<Self>, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		let mut read_dir = fs::read_dir(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		let this = Arc::new(Self {
			libraries_dir: libraries_dir.clone(),
			libraries: Default::default(),
			thumbnail_remover: ThumbnailRemoverActor::new(get_thumbnails_directory(&node)),
			node,
		});

		while let Some(entry) = read_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?
		{
			let config_path = entry.path();
			if config_path
				.extension()
				.map(|ext| ext == "sdlibrary")
				.unwrap_or(false)
				&& entry
					.metadata()
					.await
					.map_err(|e| FileIOError::from((&config_path, e)))?
					.is_file()
			{
				let Some(Ok(library_id)) = config_path
				.file_stem()
				.and_then(|v| v.to_str().map(Uuid::from_str))
			else {
				warn!(
					"Attempted to load library from path '{}' \
					but it has an invalid filename. Skipping...",
					config_path.display()
				);
					continue;
			};

				let db_path = config_path.with_extension("db");
				match fs::metadata(&db_path).await {
					Ok(_) => {}
					Err(e) if e.kind() == io::ErrorKind::NotFound => {
						warn!(
					"Found library '{}' but no matching database file was found. Skipping...",
						config_path.display()
					);
						continue;
					}
					Err(e) => return Err(FileIOError::from((db_path, e)).into()),
				}

				this.load(library_id, &db_path, config_path, None, true)
					.await?;
			}
		}

		Ok(this)
	}

	pub fn thumbnail_remover_proxy(&self) -> ThumbnailRemoverActorProxy {
		self.thumbnail_remover.proxy()
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(
		self: &Arc<Self>,
		name: LibraryName,
		description: Option<String>,
		node_cfg: NodeConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		self.create_with_uuid(Uuid::new_v4(), name, description, node_cfg, true)
			.await
	}

	pub(crate) async fn create_with_uuid(
		self: &Arc<Self>,
		id: Uuid,
		name: LibraryName,
		description: Option<String>,
		node_cfg: NodeConfig,
		should_seed: bool,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		if name.as_ref().is_empty() || name.as_ref().chars().all(|x| x.is_whitespace()) {
			return Err(LibraryManagerError::InvalidConfig(
				"name cannot be empty".to_string(),
			));
		}

		let config = LibraryConfig {
			name,
			description,
			instance_id: 0, // First instance will always be zero
		};

		let config_path = self.libraries_dir.join(format!("{id}.sdlibrary"));
		config.save(&config_path)?;

		debug!(
			"Created library '{}' config at '{}'",
			id,
			config_path.display()
		);

		let now = Utc::now().fixed_offset();
		let library = self
			.load(
				id,
				self.libraries_dir.join(format!("{id}.db")),
				config_path,
				Some(instance::Create {
					pub_id: Uuid::new_v4().as_bytes().to_vec(),
					identity: Identity::new().to_bytes(),
					node_id: node_cfg.id.as_bytes().to_vec(),
					node_name: node_cfg.name.clone(),
					node_platform: Platform::current() as i32,
					last_seen: now,
					date_created: now,
					// timestamp: Default::default(), // TODO: Source this properly!
					_params: vec![instance::id::set(config.instance_id)],
				}),
				should_seed,
			)
			.await?;

		debug!("Loaded library '{id:?}'");

		if should_seed {
			tag::seed::new_library(&library).await?;
			indexer::rules::seed::new_or_existing_library(&library).await?;
			debug!("Seeded library '{id:?}'");
		}

		invalidate_query!(library, "library.list");

		Ok(LibraryConfigWrapped { uuid: id, config })
	}

	pub(crate) async fn get_all_libraries(&self) -> Vec<Arc<Library>> {
		self.libraries.read().await.clone()
	}

	pub(crate) async fn get_all_libraries_config(&self) -> Vec<LibraryConfigWrapped> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|lib| LibraryConfigWrapped {
				config: lib.config.clone(),
				uuid: lib.id,
			})
			.collect()
	}

	// pub(crate) async fn get_all_instances(&self) -> Vec<instance::Data> {
	// 	vec![] // TODO: Cache in memory
	// }

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<LibraryName>,
		description: MaybeUndefined<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let mut libraries = self.libraries.write().await;
		let library = libraries
			.iter_mut()
			.find(|lib| lib.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		// update the library
		let mut config = library.config.clone();
		if let Some(name) = name {
			config.name = name;
		}
		match description {
			MaybeUndefined::Undefined => {}
			MaybeUndefined::Null => config.description = None,
			MaybeUndefined::Value(description) => config.description = Some(description),
		}

		LibraryConfig::save(&config, &self.libraries_dir.join(format!("{id}.sdlibrary")))?;

		invalidate_query!(library, "library.list");

		for library in libraries.iter() {
			for location in library
				.db
				.location()
				.find_many(vec![])
				.exec()
				.await
				.unwrap_or_else(|e| {
					error!(
						"Failed to get locations from database for location manager: {:#?}",
						e
					);
					vec![]
				}) {
				if let Err(e) = self
					.node
					.location_manager
					.add(location.id, library.clone())
					.await
				{
					error!("Failed to add location to location manager: {:#?}", e);
				}
			}
		}

		Ok(())
	}

	pub async fn delete(&self, id: Uuid) -> Result<(), LibraryManagerError> {
		let mut libraries_write_guard = self.libraries.write().await;

		// As we're holding a write lock here, we know that our index can't change before removal.
		let library_idx = libraries_write_guard
			.iter()
			.position(|l| l.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		let library_id = libraries_write_guard[library_idx].id;

		let db_path = self.libraries_dir.join(format!("{}.db", library_id));
		let sd_lib_path = self.libraries_dir.join(format!("{}.sdlibrary", library_id));

		try_join!(
			async {
				fs::remove_file(&db_path)
					.await
					.map_err(|e| LibraryManagerError::FileIO(FileIOError::from((db_path, e))))
			},
			async {
				fs::remove_file(&sd_lib_path)
					.await
					.map_err(|e| LibraryManagerError::FileIO(FileIOError::from((sd_lib_path, e))))
			},
		)?;

		self.thumbnail_remover.remove_library(id).await;

		// We only remove here after files deletion
		let library = libraries_write_guard.remove(library_idx);

		info!("Removed Library <id='{library_id}'>");

		invalidate_query!(library, "library.list");

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub async fn get_library(&self, library_id: Uuid) -> Option<Arc<Library>> {
		self.libraries
			.read()
			.await
			.iter()
			.find(|lib| lib.id == library_id)
			.map(Clone::clone)
	}

	/// load the library from a given path
	async fn load(
		self: &Arc<Self>,
		id: Uuid,
		db_path: impl AsRef<Path>,
		config_path: PathBuf,
		create: Option<instance::Create>,
		should_seed: bool,
	) -> Result<Arc<Library>, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let db_url = format!(
			"file:{}?socket_timeout=15&connection_limit=1",
			db_path.as_os_str().to_str().ok_or_else(|| {
				LibraryManagerError::NonUtf8Path(NonUtf8PathError(db_path.into()))
			})?
		);
		let db = Arc::new(db::load_and_migrate(&db_url).await?);

		if let Some(create) = create {
			create.to_query(&db).exec().await?;
		}

		let node_config = self.node.config.get().await;
		let config =
			LibraryConfig::load_and_migrate(&config_path, &(node_config.clone(), db.clone()))
				.await?;

		let instance = db
			.instance()
			.find_unique(instance::id::equals(config.instance_id))
			.exec()
			.await?
			.ok_or_else(|| {
				LibraryManagerError::CurrentInstanceNotFound(config.instance_id.to_string())
			})?;
		let identity = Arc::new(Identity::from_bytes(&instance.identity)?);

		let instance_id = Uuid::from_slice(&instance.pub_id)?;
		let curr_platform = Platform::current() as i32;
		let instance_node_id = Uuid::from_slice(&instance.node_id)?;
		if instance_node_id != node_config.id
			|| instance.node_platform != curr_platform
			|| instance.node_name != node_config.name
		{
			info!(
				"Detected that the library '{}' has changed node from '{}' to '{}'. Reconciling node data...",
				id, instance_node_id, node_config.id
			);

			db.instance()
				.update(
					instance::id::equals(instance.id),
					vec![
						instance::node_id::set(node_config.id.as_bytes().to_vec()),
						instance::node_platform::set(curr_platform),
						instance::node_name::set(node_config.name),
					],
				)
				.exec()
				.await?;
		}

		// TODO: Move this reconciliation into P2P and do reconciliation of both local and remote nodes.

		// let key_manager = Arc::new(KeyManager::new(vec![]).await?);
		// seed_keymanager(&db, &key_manager).await?;

		let library = Arc::new(Library::new(
			id,
			instance_id,
			config,
			identity,
			// key_manager,
			db,
			self.clone(),
		));

		self.thumbnail_remover.new_library(&library).await;
		self.libraries.write().await.push(Arc::clone(&library));

		if should_seed {
			library.orphan_remover.invoke().await;
			indexer::rules::seed::new_or_existing_library(&library).await?;
		}

		for location in library
			.db
			.location()
			.find_many(vec![
				// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
				location::instance_id::equals(Some(instance.id)),
			])
			.exec()
			.await?
		{
			if let Err(e) = library
				.node
				.location_manager
				.add(location.id, library.clone())
				.await
			{
				error!("Failed to watch location on startup: {e}");
			};
		}

		if let Err(e) = library.node.job_manager.clone().cold_resume(&library).await {
			error!("Failed to resume jobs for library. {:#?}", e);
		}

		Ok(library)
	}
}
