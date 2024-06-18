use crate::{api::CoreEvent, cloud, sync, Node};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_heavy_lifting::media_processor::ThumbnailKind;
use sd_core_prisma_helpers::{file_path_to_full_path, CasId};

use sd_p2p::Identity;
use sd_prisma::prisma::{file_path, location, PrismaClient};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	path::{Path, PathBuf},
	sync::Arc,
};

use tokio::{fs, io, sync::broadcast, sync::RwLock};
use tracing::warn;
use uuid::Uuid;

use super::{LibraryConfig, LibraryManagerError};

// TODO: Finish this
// pub enum LibraryNew {
// 	InitialSync,
// 	Encrypted,
// 	Loaded(LoadedLibrary),
//  Deleting,
// }

pub struct Library {
	/// id holds the ID of the current library.
	pub id: Uuid,
	/// config holds the configuration of the current library.
	/// KEEP PRIVATE: Access through `Self::config` method.
	config: RwLock<LibraryConfig>,
	/// db holds the database client for the current library.
	pub db: Arc<PrismaClient>,
	pub sync: Arc<sync::Manager>,
	pub cloud: cloud::State,
	/// key manager that provides encryption keys to functions that require them
	// pub key_manager: Arc<KeyManager>,
	/// p2p identity
	pub identity: Arc<Identity>,
	// pub orphan_remover: OrphanRemoverActor,
	// The UUID which matches `config.instance_id`'s primary key.
	pub instance_uuid: Uuid,

	do_cloud_sync: broadcast::Sender<()>,
	pub env: Arc<crate::env::Env>,

	// Look, I think this shouldn't be here but our current invalidation system needs it.
	// TODO(@Oscar): Get rid of this with the new invalidation system.
	event_bus_tx: broadcast::Sender<CoreEvent>,

	pub actors: Arc<sd_actors::Actors>,
}

impl Debug for Library {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// Rolling out this implementation because `NodeContext` contains a DynJob which is
		// troublesome to implement Debug trait
		f.debug_struct("LibraryContext")
			.field("id", &self.id)
			.field("instance_uuid", &self.instance_uuid)
			.field("config", &self.config)
			.field("db", &self.db)
			.finish()
	}
}

impl Library {
	#[allow(clippy::too_many_arguments)]
	pub async fn new(
		id: Uuid,
		config: LibraryConfig,
		instance_uuid: Uuid,
		identity: Arc<Identity>,
		db: Arc<PrismaClient>,
		node: &Arc<Node>,
		sync: Arc<sync::Manager>,
		cloud: cloud::State,
		do_cloud_sync: broadcast::Sender<()>,
		actors: Arc<sd_actors::Actors>,
	) -> Arc<Self> {
		Arc::new(Self {
			id,
			config: RwLock::new(config),
			sync,
			cloud,
			db: db.clone(),
			// key_manager,
			identity,
			// orphan_remover: OrphanRemoverActor::spawn(db),
			instance_uuid,
			do_cloud_sync,
			env: node.env.clone(),
			event_bus_tx: node.event_bus.0.clone(),
			actors,
		})
	}

	pub async fn config(&self) -> LibraryConfig {
		self.config.read().await.clone()
	}

	pub async fn update_config(
		&self,
		update_fn: impl FnOnce(&mut LibraryConfig),
		config_path: impl AsRef<Path>,
	) -> Result<(), LibraryManagerError> {
		let mut config = self.config.write().await;

		update_fn(&mut config);

		config.save(config_path).await.map_err(Into::into)
	}

	// TODO: Remove this once we replace the old invalidation system
	pub(crate) fn emit(&self, event: CoreEvent) {
		if let Err(e) = self.event_bus_tx.send(event) {
			warn!(?e, "Error sending event to event bus;");
		}
	}

	pub async fn thumbnail_exists(
		&self,
		node: &Node,
		cas_id: &CasId<'_>,
	) -> Result<bool, FileIOError> {
		let thumb_path =
			ThumbnailKind::Indexed(self.id).compute_path(node.config.data_directory(), cas_id);

		match fs::metadata(&thumb_path).await {
			Ok(_) => Ok(true),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
			Err(e) => Err(FileIOError::from((thumb_path, e))),
		}
	}

	/// Returns the full path of a file
	pub async fn get_file_paths(
		&self,
		ids: Vec<file_path::id::Type>,
	) -> Result<HashMap<file_path::id::Type, Option<PathBuf>>, LibraryManagerError> {
		let mut out = ids
			.iter()
			.copied()
			.map(|id| (id, None))
			.collect::<HashMap<_, _>>();

		out.extend(
			self.db
				.file_path()
				.find_many(vec![
					// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
					file_path::location::is(vec![location::instance_id::equals(Some(
						self.config().await.instance_id,
					))]),
					file_path::id::in_vec(ids),
				])
				.select(file_path_to_full_path::select())
				.exec()
				.await?
				.into_iter()
				.flat_map(|file_path| {
					let location = maybe_missing(&file_path.location, "file_path.location")?;

					Ok::<_, LibraryManagerError>((
						file_path.id,
						location
							.path
							.as_ref()
							.map(|location_path| {
								IsolatedFilePathData::try_from((location.id, &file_path))
									.map(|data| Path::new(&location_path).join(data))
							})
							.transpose()?,
					))
				}),
		);

		Ok(out)
	}

	pub fn do_cloud_sync(&self) {
		if let Err(e) = self.do_cloud_sync.send(()) {
			warn!(?e, "Error sending cloud resync message;");
		}
	}
}
