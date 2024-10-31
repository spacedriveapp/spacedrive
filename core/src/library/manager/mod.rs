use crate::{
	api::{utils::InvalidateOperationEvent, CoreEvent},
	invalidate_query,
	location::metadata::{LocationMetadataError, SpacedriveLocationMetadataFile},
	object::tag,
	p2p,
	util::{mpscrr, MaybeUndefined},
	Node,
};

use sd_core_sync::{SyncEvent, SyncManager};

use sd_p2p::{Identity, RemoteIdentity};
use sd_prisma::{
	prisma::{self, device, instance, location, PrismaClient},
	prisma_sync,
};
use sd_sync::ModelId;
use sd_utils::{
	db,
	error::{FileIOError, NonUtf8PathError},
};

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use chrono::Utc;
use futures_concurrency::future::{Join, TryJoin};
use prisma_client_rust::Raw;
use tokio::{
	fs, io, spawn,
	sync::{broadcast, RwLock},
};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use super::{Library, LibraryConfig, LibraryName};

mod error;

pub mod pragmas;

use pragmas::configure_pragmas;

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
	pub libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<HashMap<Uuid, Arc<Library>>>,
	// Transmit side of `self.rx` channel
	tx: mpscrr::Sender<LibraryManagerEvent, ()>,
	/// A channel for receiving events from the library manager.
	pub rx: mpscrr::Receiver<LibraryManagerEvent, ()>,
	pub emit_messages_flag: Arc<AtomicBool>,
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
			emit_messages_flag: Arc::new(AtomicBool::new(false)),
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
						config_path = %config_path.display(),
						"Attempted to load library from path \
						but it has an invalid filename. Skipping...;",
					);
					continue;
				};

				let db_path = config_path.with_extension("db");
				match fs::metadata(&db_path).await {
					Ok(_) => {}
					Err(e) if e.kind() == io::ErrorKind::NotFound => {
						warn!(
							config_path = %config_path.display(),
							"Found library but no matching database file was found. Skipping...;",
						);

						continue;
					}
					Err(e) => return Err(FileIOError::from((db_path, e)).into()),
				}

				let _library_arc = self
					.load(library_id, &db_path, config_path, None, None, true, node)
					.await?;

				// FIX-ME: Linux releases crashes with *** stack smashing detected *** if spawn_volume_watcher is enabled
				// No idea why, but this will be irrelevant after the UDisk API is implemented, so let's leave it disabled for now
				#[cfg(not(target_os = "linux"))]
				{
					use crate::volume::watcher::spawn_volume_watcher;
					spawn_volume_watcher(_library_arc.clone());
				}
			}
		}

		Ok(())
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub async fn create(
		self: &Arc<Self>,
		name: LibraryName,
		description: Option<String>,
		node: &Arc<Node>,
	) -> Result<Arc<Library>, LibraryManagerError> {
		self.create_with_uuid(Uuid::now_v7(), name, description, true, None, node)
			.await
	}

	#[instrument(skip(self, instance, node), err)]
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

		let config_path = self.libraries_dir.join(format!("{id}.sdlibrary"));

		let config = LibraryConfig::new(
			name,
			description,
			// First instance will be zero
			0,
			&config_path,
		)
		.await?;

		debug!(
			config_path = %config_path.display(),
			"Created library;",
		);

		let node_cfg = node.config.get().await;
		let now = Utc::now().fixed_offset();
		let library = self
			.load(
				id,
				self.libraries_dir.join(format!("{id}.db")),
				config_path,
				Some(device::Create {
					pub_id: node_cfg.id.to_db(),
					_params: vec![
						device::name::set(Some(node_cfg.name.clone())),
						device::os::set(Some(node_cfg.os as i32)),
						device::hardware_model::set(Some(node_cfg.hardware_model as i32)),
						device::date_created::set(Some(now)),
					],
				}),
				Some({
					let identity = Identity::new();
					let mut create = instance.unwrap_or_else(|| instance::Create {
						pub_id: Uuid::now_v7().as_bytes().to_vec(),
						remote_identity: identity.to_remote_identity().get_bytes().to_vec(),
						node_id: node_cfg.id.to_db(),
						last_seen: now,
						date_created: now,
						_params: vec![
							instance::identity::set(Some(identity.to_bytes())),
							instance::metadata::set(Some(
								serde_json::to_vec(&node.p2p.peer_metadata())
									.expect("invalid node metadata"),
							)),
						],
					});
					create._params.push(instance::id::set(config.instance_id));
					create
				}),
				should_seed,
				node,
			)
			.await?;

		debug!("Loaded library");

		if should_seed {
			tag::seed::new_library(&library).await?;
			sd_core_indexer_rules::seed::new_or_existing_library(&library.db).await?;
			debug!("Seeded library");
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
		cloud_id: MaybeUndefined<String>,
		enable_sync: Option<bool>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let libraries = self.libraries.read().await;
		let library = Arc::clone(
			libraries
				.get(&id)
				.ok_or(LibraryManagerError::LibraryNotFound)?,
		);

		library
			.update_config(|config| {
				// update the library
				if let Some(name) = name {
					config.name = name;
				}
				match description {
					MaybeUndefined::Undefined => {}
					MaybeUndefined::Null => config.description = None,
					MaybeUndefined::Value(description) => config.description = Some(description),
				}
				match cloud_id {
					MaybeUndefined::Undefined => {}
					MaybeUndefined::Null => config.cloud_id = None,
					MaybeUndefined::Value(cloud_id) => config.cloud_id = Some(cloud_id),
				}
				match enable_sync {
					None => {}
					Some(value) => config
						.generate_sync_operations
						.store(value, Ordering::SeqCst),
				}
			})
			.await?;

		self.tx
			.emit(LibraryManagerEvent::Edit(Arc::clone(&library)))
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

		if let Ok(location_paths) = library
			.db
			.location()
			.find_many(vec![])
			.select(location::select!({ path }))
			.exec()
			.await
			.map(|locations| locations.into_iter().filter_map(|location| location.path))
			.map_err(|e| error!(?e, "Failed to fetch locations for library deletion;"))
		{
			location_paths
				.map(|location_path| async move {
					if let Some(mut sd_metadata) =
						SpacedriveLocationMetadataFile::try_load(location_path).await?
					{
						sd_metadata.remove_library(*id).await?;
					}

					Ok::<_, LocationMetadataError>(())
				})
				.collect::<Vec<_>>()
				.join()
				.await
				.into_iter()
				.for_each(|res| {
					if let Err(e) = res {
						error!(?e, "Failed to remove library from location metadata;");
					}
				});
		}

		let db_path = self.libraries_dir.join(format!("{}.db", library.id));
		let sd_lib_path = self.libraries_dir.join(format!("{}.sdlibrary", library.id));

		(
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
		)
			.try_join()
			.await?;

		// We only remove here after files deletion
		let library = libraries_write_guard
			.remove(id)
			.expect("we have exclusive access and checked it exists!");

		info!(%library.id, "Removed Library;");

		invalidate_query!(library, "library.list");

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub async fn get_library(&self, library_id: &Uuid) -> Option<Arc<Library>> {
		self.libraries.read().await.get(library_id).cloned()
	}

	// will return the library context for the given instance
	pub async fn get_library_for_instance(
		&self,
		instance: &RemoteIdentity,
	) -> Option<Arc<Library>> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|(_, library)| async move {
				library
					.db
					.instance()
					.find_many(vec![instance::remote_identity::equals(
						instance.get_bytes().to_vec(),
					)])
					.exec()
					.await
					.ok()
					.iter()
					.flatten()
					.filter_map(|i| RemoteIdentity::from_bytes(&i.remote_identity).ok())
					.any(|i| i == *instance)
					.then(|| Arc::clone(library))
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.find_map(|v| v)
	}

	// get_ctx will return the library context for the given library id.
	pub async fn hash_library(&self, library_id: &Uuid) -> bool {
		self.libraries.read().await.get(library_id).is_some()
	}

	#[allow(clippy::too_many_arguments)] // TODO: remove this when we remove instance stuff
	#[instrument(
		skip_all,
		fields(
			library_id = %id,
			db_path = %db_path.as_ref().display(),
			config_path = %config_path.as_ref().display(),
			%should_seed,
		),
		err,
	)]
	/// load the library from a given path.
	pub async fn load(
		self: &Arc<Self>,
		id: Uuid,
		db_path: impl AsRef<Path>,
		config_path: impl AsRef<Path>,
		maybe_create_device: Option<device::Create>,
		maybe_create_instance: Option<instance::Create>, // Deprecated
		should_seed: bool,
		node: &Arc<Node>,
	) -> Result<Arc<Library>, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let config_path = config_path.as_ref();

		let db_url = format!(
			"file:{}?socket_timeout=15&connection_limit=1",
			db_path.as_os_str().to_str().ok_or_else(|| {
				LibraryManagerError::NonUtf8Path(NonUtf8PathError(db_path.into()))
			})?
		);
		let db = Arc::new(db::load_and_migrate(&db_url).await?);

		// Configure database
		configure_pragmas(&db).await?;
		special_sync_indexes(&db).await?;

		if let Some(create) = maybe_create_device {
			create.to_query(&db).exec().await?;
		}

		// TODO: remove instances from locations
		if let Some(create) = maybe_create_instance {
			create.to_query(&db).exec().await?;
		}

		let node_config = node.config.get().await;
		let device_pub_id = node_config.id.clone();
		let config = LibraryConfig::load(config_path, &node_config, &db).await?;

		let instances = db.instance().find_many(vec![]).exec().await?;

		let instance = instances
			.iter()
			.find(|i| i.id == config.instance_id)
			.ok_or_else(|| {
				LibraryManagerError::CurrentInstanceNotFound(config.instance_id.to_string())
			})?
			.clone();

		let devices = db.device().find_many(vec![]).exec().await?;

		let device_pub_id_to_db = device_pub_id.to_db();
		if !devices
			.iter()
			.any(|device| device.pub_id == device_pub_id_to_db)
		{
			return Err(LibraryManagerError::CurrentDeviceNotFound(device_pub_id));
		}

		let identity = match instance.identity.as_ref() {
			Some(b) => Arc::new(Identity::from_bytes(b)?),
			// We are not this instance, so we don't have the private key.
			None => return Err(LibraryManagerError::InvalidIdentity),
		};

		let instance_id = Uuid::from_slice(&instance.pub_id)?;
		let curr_metadata: Option<HashMap<String, String>> = instance
			.metadata
			.as_ref()
			.map(|metadata| serde_json::from_slice(metadata).expect("invalid metadata"));
		let instance_node_id = Uuid::from_slice(&instance.node_id)?;
		let instance_node_remote_identity = instance
			.node_remote_identity
			.as_ref()
			.and_then(|v| RemoteIdentity::from_bytes(v).ok());
		if instance_node_id != Uuid::from(&node_config.id)
			|| instance_node_remote_identity != Some(node_config.identity.to_remote_identity())
			|| curr_metadata != Some(node.p2p.peer_metadata())
		{
			info!(
				old_node_id = %instance_node_id,
				new_node_id = %node_config.id,
				"Detected that the library has changed nodes. Reconciling node data...",
			);

			// ensure

			db.instance()
				.update(
					instance::id::equals(instance.id),
					vec![
						instance::node_id::set(node_config.id.to_db()),
						instance::node_remote_identity::set(Some(
							node_config
								.identity
								.to_remote_identity()
								.get_bytes()
								.to_vec(),
						)),
						instance::metadata::set(Some(
							serde_json::to_vec(&node.p2p.peer_metadata())
								.expect("invalid peer metadata"),
						)),
					],
				)
				.select(instance::select!({ id }))
				.exec()
				.await?;
		}

		// TODO: Move this reconciliation into P2P and do reconciliation of both local and remote nodes.

		let (sync, sync_rx) = SyncManager::with_existing_devices(
			Arc::clone(&db),
			&device_pub_id,
			Arc::clone(&config.generate_sync_operations),
			&devices,
		)
		.await?;

		let library = Library::new(id, config, instance_id, identity, db, node, sync).await;

		// This is an exception. Generally subscribe to this by `self.tx.subscribe`.
		spawn(sync_rx_actor(library.clone(), node.clone(), sync_rx));

		self.tx
			.emit(LibraryManagerEvent::Load(library.clone()))
			.await;

		self.libraries
			.write()
			.await
			.insert(library.id, Arc::clone(&library));

		if should_seed {
			// library.orphan_remover.invoke().await;
			sd_core_indexer_rules::seed::new_or_existing_library(&library.db).await?;
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
				error!(?e, "Failed to watch location on startup;");
			};
		}

		if let Err(e) = node.old_jobs.clone().cold_resume(node, &library).await {
			error!(?e, "Failed to resume jobs for library;");
		}

		Ok(library)
	}

	pub async fn update_instances(&self, library: Arc<Library>) {
		self.tx
			.emit(LibraryManagerEvent::InstancesModified(library))
			.await;
	}

	pub async fn update_instances_by_id(&self, library_id: Uuid) {
		let Some(library) = self.libraries.read().await.get(&library_id).cloned() else {
			warn!("Failed to find instance to update by id");
			return;
		};

		self.tx
			.emit(LibraryManagerEvent::InstancesModified(library))
			.await;
	}
}

async fn sync_rx_actor(
	library: Arc<Library>,
	node: Arc<Node>,
	mut sync_rx: broadcast::Receiver<SyncEvent>,
) {
	loop {
		let Ok(msg) = sync_rx.recv().await else {
			continue;
		};

		match msg {
			// TODO: Any sync event invalidates the entire React Query cache this is a hacky workaround until the new invalidation system.
			SyncEvent::Ingested => node.emit(CoreEvent::InvalidateOperation(
				InvalidateOperationEvent::all(),
			)),
			SyncEvent::Created => {
				p2p::sync::originator(library.clone(), &library.sync, &node.p2p).await
			}
		}
	}
}

async fn special_sync_indexes(db: &PrismaClient) -> Result<(), LibraryManagerError> {
	async fn create_index(
		db: &PrismaClient,
		model_id: ModelId,
		model_name: &str,
	) -> Result<(), LibraryManagerError> {
		db._execute_raw(Raw::new(
			&format!(
				"CREATE INDEX IF NOT EXISTS partial_index_model_{model_name} \
				ON crdt_operation(model,record_id,kind,timestamp) \
				WHERE model = {model_id}
				"
			),
			vec![],
		))
		.exec()
		.await?;

		debug!(model_name, "Created sync partial index");

		Ok(())
	}

	for (model_id, model_name) in [
		(prisma_sync::device::MODEL_ID, prisma::device::NAME),
		(
			prisma_sync::storage_statistics::MODEL_ID,
			prisma::storage_statistics::NAME,
		),
		(prisma_sync::tag::MODEL_ID, prisma::tag::NAME),
		(prisma_sync::location::MODEL_ID, prisma::location::NAME),
		(prisma_sync::object::MODEL_ID, prisma::object::NAME),
		(prisma_sync::label::MODEL_ID, prisma::label::NAME),
		(prisma_sync::exif_data::MODEL_ID, prisma::exif_data::NAME),
		(prisma_sync::file_path::MODEL_ID, prisma::file_path::NAME),
		(
			prisma_sync::tag_on_object::MODEL_ID,
			prisma::tag_on_object::NAME,
		),
		(
			prisma_sync::label_on_object::MODEL_ID,
			prisma::label_on_object::NAME,
		),
	] {
		// Creating indexes sequentially just in case
		create_index(db, model_id, model_name).await?;
	}

	Ok(())
}
