use crate::{
	invalidate_query,
	location::indexer,
	node::Platform,
	object::tag,
	p2p::IdentityOrRemoteIdentity,
	prisma::location,
	sync,
	util::{
		db,
		error::{FileIOError, NonUtf8PathError},
		migrator::Migrate,
		mpscrr, MaybeUndefined,
	},
	Node,
};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use chrono::Utc;
use sd_core_sync::SyncMessage;
use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::instance;
use tokio::{fs, io, sync::RwLock, try_join};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{Library, LibraryConfig, LibraryName};

mod error;

pub use error::*;

/// Event that is emitted to subscribers of the library manager.
#[derive(Debug, Clone)]
pub enum LibraryManagerEvent {
	Load(Arc<Library>),
	Edit(Arc<Library>),
	// TODO(@Oscar): Replace this with pairing -> ready state transitions
	InstancesModified(Arc<Library>),
	Delete(Arc<Library>),
}

/// is a singleton that manages all libraries for a node.
pub struct Libraries {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<HashMap<Uuid, Arc<Library>>>,
	// Transmit side of `self.rx` channel
	tx: mpscrr::Sender<LibraryManagerEvent, ()>,
	/// A channel for receiving events from the library manager.
	pub rx: mpscrr::Receiver<LibraryManagerEvent, ()>,
}

impl Libraries {
	pub(crate) async fn new(libraries_dir: PathBuf) -> Result<Arc<Self>, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		let (tx, rx) = mpscrr::unbounded_channel();
		Ok(Arc::new(Self {
			libraries_dir,
			libraries: Default::default(),
			tx,
			rx,
		}))
	}

	/// Loads the initial libraries from disk.
	///
	/// `Arc<LibraryManager>` is constructed and passed to other managers for them to subscribe (`self.rx.subscribe`) then this method is run to load the initial libraries and trigger the subscriptions.
	pub async fn init(self: &Arc<Self>, node: &Arc<Node>) -> Result<(), LibraryManagerError> {
		let mut read_dir = fs::read_dir(&self.libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&self.libraries_dir, e)))?;

		while let Some(entry) = read_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((&self.libraries_dir, e)))?
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
						warn!("Found library '{}' but no matching database file was found. Skipping...", config_path.display());
						continue;
					}
					Err(e) => return Err(FileIOError::from((db_path, e)).into()),
				}

				self.load(library_id, &db_path, config_path, None, true, node)
					.await?;
			}
		}

		Ok(())
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(
		self: &Arc<Self>,
		name: LibraryName,
		description: Option<String>,
		node: &Arc<Node>,
	) -> Result<Arc<Library>, LibraryManagerError> {
		self.create_with_uuid(Uuid::new_v4(), name, description, true, None, node)
			.await
	}

	pub(crate) async fn create_with_uuid(
		self: &Arc<Self>,
		id: Uuid,
		name: LibraryName,
		description: Option<String>,
		should_seed: bool,
		// `None` will fallback to default as library must be created with at least one instance
		instance: Option<instance::Create>,
		node: &Arc<Node>,
	) -> Result<Arc<Library>, LibraryManagerError> {
		if name.as_ref().is_empty() || name.as_ref().chars().all(|x| x.is_whitespace()) {
			return Err(LibraryManagerError::InvalidConfig(
				"name cannot be empty".to_string(),
			));
		}

		let config = LibraryConfig {
			name,
			description,
			// First instance will be zero
			instance_id: 0,
		};

		let config_path = self.libraries_dir.join(format!("{id}.sdlibrary"));
		config.save(&config_path)?;

		debug!(
			"Created library '{}' config at '{}'",
			id,
			config_path.display()
		);

		let node_cfg = node.config.get().await;
		let now = Utc::now().fixed_offset();
		let library = self
			.load(
				id,
				self.libraries_dir.join(format!("{id}.db")),
				config_path,
				Some({
					let mut create = instance.unwrap_or_else(|| instance::Create {
						pub_id: Uuid::new_v4().as_bytes().to_vec(),
						identity: IdentityOrRemoteIdentity::Identity(Identity::new()).to_bytes(),
						node_id: node_cfg.id.as_bytes().to_vec(),
						node_name: node_cfg.name.clone(),
						node_platform: Platform::current() as i32,
						last_seen: now,
						date_created: now,
						_params: vec![],
					});
					create._params.push(instance::id::set(config.instance_id));
					create
				}),
				should_seed,
				node,
			)
			.await?;

		debug!("Loaded library '{id:?}'");

		if should_seed {
			tag::seed::new_library(&library).await?;
			indexer::rules::seed::new_or_existing_library(&library).await?;
			debug!("Seeded library '{id:?}'");
		}

		invalidate_query!(library, "library.list");

		Ok(library)
	}

	/// `LoadedLibrary.id` can be used to get the library's id.
	pub async fn get_all(&self) -> Vec<Arc<Library>> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|v| v.1.clone())
			.collect()
	}

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<LibraryName>,
		description: MaybeUndefined<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let libraries = self.libraries.write().await;
		let library = libraries
			.get(&id)
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

		self.tx
			.emit(LibraryManagerEvent::Edit(library.clone()))
			.await;
		invalidate_query!(library, "library.list");

		Ok(())
	}

	pub async fn delete(&self, id: &Uuid) -> Result<(), LibraryManagerError> {
		// As we're holding a write lock here, we know nothing will change during this function
		let mut libraries_write_guard = self.libraries.write().await;

		// TODO: Library go into "deletion" state until it's finished!

		let library = libraries_write_guard
			.get(id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		self.tx
			.emit(LibraryManagerEvent::Delete(library.clone()))
			.await;

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

		// We only remove here after files deletion
		let library = libraries_write_guard
			.remove(id)
			.expect("we have exclusive access and checked it exists!");

		info!("Removed Library <id='{}'>", library.id);

		invalidate_query!(library, "library.list");

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub async fn get_library(&self, library_id: &Uuid) -> Option<Arc<Library>> {
		self.libraries.read().await.get(library_id).cloned()
	}

	// get_ctx will return the library context for the given library id.
	pub async fn hash_library(&self, library_id: &Uuid) -> bool {
		self.libraries.read().await.get(library_id).is_some()
	}

	/// load the library from a given path
	async fn load(
		self: &Arc<Self>,
		id: Uuid,
		db_path: impl AsRef<Path>,
		config_path: PathBuf,
		create: Option<instance::Create>,
		should_seed: bool,
		node: &Arc<Node>,
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

		let node_config = node.config.get().await;
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
		let identity = Arc::new(
			match IdentityOrRemoteIdentity::from_bytes(&instance.identity)? {
				IdentityOrRemoteIdentity::Identity(identity) => identity,
				IdentityOrRemoteIdentity::RemoteIdentity(_) => {
					return Err(LibraryManagerError::InvalidIdentity)
				}
			},
		);

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

		let mut sync = sync::Manager::new(&db, instance_id);

		let library = Library::new(
			id,
			config,
			identity,
			// key_manager,
			db,
			&node,
			Arc::new(sync.manager),
		)
		.await;

		// This is an exception. Generally subscribe to this by `self.tx.subscribe`.
		tokio::spawn({
			let library = library.clone();
			let node = node.clone();

			async move {
				loop {
					tokio::select! {
						req = sync.ingest_rx.recv() => {
							use sd_core_sync::ingest;

							let Some(req) = req else { continue; };

							const OPS_PER_REQUEST: u32 = 100;

							match req {
								ingest::Request::Messages { mut tunnel, timestamps } => {
									let ops = node.nlm.request_ops(
										&mut tunnel,
										sd_core_sync::GetOpsArgs { clocks: timestamps, count: OPS_PER_REQUEST },
									).await;

									library.sync.ingest
										.event_tx
										.send(ingest::Event::Messages(ingest::MessagesEvent {
											tunnel,
											instance_id: library.sync.instance,
											has_more: ops.len() == OPS_PER_REQUEST as usize,
											messages: ops,
										}))
										.await
										.expect("TODO: Handle ingest channel closed, so we don't loose ops");
								},
								_ => {}
							}
						},
						msg = sync.rx.recv() => {
							if let Ok(op) = msg {
								let SyncMessage::Created = op else { continue; };

								node.nlm.alert_new_ops(id, &library.sync).await;
							}
						},
					}
				}
			}
		});

		self.tx
			.emit(LibraryManagerEvent::Load(library.clone()))
			.await;

		self.libraries
			.write()
			.await
			.insert(library.id, Arc::clone(&library));

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
			if let Err(e) = node.locations.add(location.id, library.clone()).await {
				error!("Failed to watch location on startup: {e}");
			};
		}

		if let Err(e) = node.jobs.clone().cold_resume(node, &library).await {
			error!("Failed to resume jobs for library. {:#?}", e);
		}

		Ok(library)
	}

	pub async fn update_instances(&self, library: Arc<Library>) {
		self.tx
			.emit(LibraryManagerEvent::InstancesModified(library))
			.await;
	}
}
