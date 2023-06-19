use crate::{
	invalidate_query,
	location::{indexer::rules, LocationManagerError},
	node::{NodeConfig, Platform},
	object::orphan_remover::OrphanRemoverActor,
	prisma::{location, node},
	sync::{SyncManager, SyncMessage},
	util::{
		db::{self, MissingFieldError},
		error::{FileIOError, NonUtf8PathError},
		migrator::{Migrate, MigratorError},
		MaybeUndefined,
	},
	NodeContext,
};

use std::{
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use chrono::Local;
use sd_p2p::spacetunnel::{Identity, IdentityErr};
use thiserror::Error;
use tokio::{
	fs, io,
	sync::{broadcast, RwLock},
	try_join,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{Library, LibraryConfig, LibraryConfigWrapped};

pub enum SubscriberEvent {
	Load(Uuid, Arc<Identity>, broadcast::Receiver<SyncMessage>),
}

impl Clone for SubscriberEvent {
	fn clone(&self) -> Self {
		match self {
			Self::Load(id, identity, receiver) => {
				Self::Load(*id, identity.clone(), receiver.resubscribe())
			}
		}
	}
}

pub trait SubscriberFn: Fn(SubscriberEvent) + Send + Sync + 'static {}
impl<F: Fn(SubscriberEvent) + Send + Sync + 'static> SubscriberFn for F {}

/// LibraryManager is a singleton that manages all libraries for a node.
pub struct LibraryManager {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<Vec<Library>>,
	/// node_context holds the context for the node which this library manager is running on.
	node_context: NodeContext,
	/// on load subscribers
	subscribers: RwLock<Vec<Box<dyn SubscriberFn>>>,
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
	IndexerRulesSeeder(#[from] rules::SeederError),
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
	#[error("current node with id '{0}' was not found in the database")]
	CurrentNodeNotFound(String),
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
		node_context: NodeContext,
	) -> Result<Arc<Self>, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		let mut libraries = Vec::new();
		let subscribers = RwLock::new(Vec::new());
		let mut read_dir = fs::read_dir(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		while let Some(entry) = read_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?
		{
			let config_path = entry.path();
			let metadata = entry
				.metadata()
				.await
				.map_err(|e| FileIOError::from((&config_path, e)))?;
			if metadata.is_file()
				&& config_path
					.extension()
					.map(|ext| ext == "sdlibrary")
					.unwrap_or(false)
			{
				let Some(Ok(library_id)) = config_path
				.file_stem()
				.and_then(|v| v.to_str().map(Uuid::from_str))
			else {
				warn!("Attempted to load library from path '{}' but it has an invalid filename. Skipping...", config_path.display());
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

				libraries.push(
					Self::load(
						library_id,
						&db_path,
						config_path,
						node_context.clone(),
						&subscribers,
						None,
					)
					.await?,
				);
			}
		}

		let this = Arc::new(Self {
			libraries: RwLock::new(libraries),
			libraries_dir,
			node_context,
			subscribers,
		});

		debug!("LibraryManager initialized");

		Ok(this)
	}

	/// subscribe to library events
	pub(crate) async fn subscribe<F: SubscriberFn>(&self, f: F) {
		self.subscribers.write().await.push(Box::new(f));
	}

	async fn emit(subscribers: &RwLock<Vec<Box<dyn SubscriberFn>>>, event: SubscriberEvent) {
		let subscribers = subscribers.read().await;
		for subscriber in subscribers.iter() {
			subscriber(event.clone());
		}
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(
		&self,
		config: LibraryConfig,
		node_cfg: NodeConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		self.create_with_uuid(Uuid::new_v4(), config, node_cfg)
			.await
	}

	pub(crate) async fn create_with_uuid(
		&self,
		id: Uuid,
		config: LibraryConfig,
		node_cfg: NodeConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		if config.name.is_empty() || config.name.chars().all(|x| x.is_whitespace()) {
			return Err(LibraryManagerError::InvalidConfig(
				"name cannot be empty".to_string(),
			));
		}

		let config_path = self.libraries_dir.join(format!("{id}.sdlibrary"));
		config.save(&config_path)?;

		debug!(
			"Created library '{}' config at '{}'",
			id,
			config_path.display()
		);

		let library = Self::load(
			id,
			self.libraries_dir.join(format!("{id}.db")),
			config_path,
			self.node_context.clone(),
			&self.subscribers,
			Some(node::Create {
				pub_id: config.node_id.as_bytes().to_vec(),
				name: node_cfg.name.clone(),
				platform: Platform::current() as i32,
				date_created: Local::now().into(),
				_params: vec![
					node::identity::set(Some(config.identity.clone())),
					node::node_peer_id::set(Some(node_cfg.keypair.peer_id().to_string())),
				],
			}),
		)
		.await?;

		debug!("Loaded library '{id:?}'");

		// Run seeders
		rules::seeder(&library.db).await?;

		debug!("Seeded library '{id:?}'");

		invalidate_query!(library, "library.list");

		self.libraries.write().await.push(library);

		debug!("Pushed library into manager '{id:?}'");

		Ok(LibraryConfigWrapped {
			uuid: id,
			config: config.into(),
		})
	}

	pub(crate) async fn get_all_libraries_config(&self) -> Vec<LibraryConfigWrapped> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|lib| LibraryConfigWrapped {
				config: lib.config.clone().into(),
				uuid: lib.id,
			})
			.collect()
	}

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<String>,
		description: MaybeUndefined<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let mut libraries = self.libraries.write().await;
		let library = libraries
			.iter_mut()
			.find(|lib| lib.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		// update the library
		if let Some(name) = name {
			library.config.name = name;
		}
		match description {
			MaybeUndefined::Undefined => {}
			MaybeUndefined::Null => library.config.description = None,
			MaybeUndefined::Value(description) => library.config.description = Some(description),
		}

		LibraryConfig::save(
			&library.config,
			&self.libraries_dir.join(format!("{id}.sdlibrary")),
		)?;

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
					.node_context
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
		let mut libraries = self.libraries.write().await;

		let library = libraries
			.iter()
			.find(|l| l.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		let db_path = self.libraries_dir.join(format!("{}.db", library.id));
		let sd_lib_path = self.libraries_dir.join(format!("{}.sdlibrary", library.id));

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

		invalidate_query!(library, "library.list");

		libraries.retain(|l| l.id != id);

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub async fn get_library(&self, library_id: Uuid) -> Option<Library> {
		self.libraries
			.read()
			.await
			.iter()
			.find(|lib| lib.id == library_id)
			.map(Clone::clone)
	}

	/// load the library from a given path
	async fn load(
		id: Uuid,
		db_path: impl AsRef<Path>,
		config_path: PathBuf,
		node_context: NodeContext,
		subscribers: &RwLock<Vec<Box<dyn SubscriberFn>>>,
		create: Option<node::Create>,
	) -> Result<Library, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let db_url = format!(
			"file:{}",
			db_path.as_os_str().to_str().ok_or_else(|| {
				LibraryManagerError::NonUtf8Path(NonUtf8PathError(db_path.into()))
			})?
		);
		let db = Arc::new(db::load_and_migrate(&db_url).await?);

		if let Some(create) = create {
			create.to_query(&db).exec().await?;
		}

		let node_config = node_context.config.get().await;
		let config = LibraryConfig::load_and_migrate(
			&config_path,
			&(node_config.id, node_config.keypair.peer_id(), db.clone()),
		)
		.await?;
		let identity = Arc::new(Identity::from_bytes(&config.identity)?);

		let node_data = db
			.node()
			.find_unique(node::pub_id::equals(node_config.id.as_bytes().to_vec()))
			.exec()
			.await?
			.ok_or_else(|| LibraryManagerError::CurrentNodeNotFound(id.to_string()))?;

		let curr_platform = Platform::current() as i32;
		if node_data.platform != curr_platform {
			info!(
				"Detected change of platform for library '{}', was previously '{}' and will change to '{}'. Reconciling node data.",
				id,
				node_data.platform,
				curr_platform
			);

			db.node()
				.update(
					node::pub_id::equals(node_data.pub_id.clone()),
					vec![
						node::platform::set(curr_platform),
						node::name::set(node_config.name.clone()),
					],
				)
				.exec()
				.await?;
		}

		if node_data.name != node_config.name {
			info!(
				"Detected change of node name for library '{}', was previously '{}' and will change to '{}'. Reconciling node data.",
				id,
				node_data.name,
				node_config.name,
			);

			db.node()
				.update(
					node::pub_id::equals(node_data.pub_id),
					vec![node::name::set(node_config.name.clone())],
				)
				.exec()
				.await?;
		}

		// TODO: Move this reconciliation into P2P and do reconciliation of both local and remote nodes.

		// let key_manager = Arc::new(KeyManager::new(vec![]).await?);
		// seed_keymanager(&db, &key_manager).await?;

		rules::seeder(&db).await?;

		let (sync_manager, sync_rx) = SyncManager::new(&db, id);

		Self::emit(
			subscribers,
			SubscriberEvent::Load(id, identity.clone(), sync_rx),
		)
		.await;

		let library = Library {
			id,
			local_id: node_data.id,
			config,
			// key_manager,
			sync: Arc::new(sync_manager),
			orphan_remover: OrphanRemoverActor::spawn(db.clone()),
			db,
			node_local_id: node_data.id,
			node_context,
			identity,
		};

		for location in library
			.db
			.location()
			.find_many(vec![location::node_id::equals(Some(node_data.id))])
			.exec()
			.await?
		{
			if let Err(e) = library
				.node_context
				.location_manager
				.add(location.id, library.clone())
				.await
			{
				error!("Failed to watch location on startup: {e}");
			};
		}

		if let Err(e) = library
			.node_context
			.jobs
			.clone()
			.cold_resume(&library)
			.await
		{
			error!("Failed to resume jobs for library. {:#?}", e);
		}

		Ok(library)
	}
}
