use crate::{
	api::CoreEvent,
	job::{IntoJob, JobInitData, JobManagerError, StatefulJob},
	location::{
		file_path_helper::{file_path_to_full_path, IsolatedFilePathData},
		LocationManager,
	},
	node::NodeConfigManager,
	object::{orphan_remover::OrphanRemoverActor, preview::get_thumbnail_path},
	prisma::{file_path, location, PrismaClient},
	sync::SyncManager,
	util::{db::maybe_missing, error::FileIOError},
	NodeContext,
};

use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_p2p::spacetunnel::Identity;
use tokio::{fs, io};
use tracing::warn;
use uuid::Uuid;

use super::{LibraryConfig, LibraryManagerError};

/// LibraryContext holds context for a library which can be passed around the application.
#[derive(Clone)]
pub struct Library {
	/// id holds the ID of the current library.
	pub id: Uuid,
	/// local_id holds the local ID of the current library.
	pub local_id: i32,
	/// config holds the configuration of the current library.
	pub config: LibraryConfig,
	/// db holds the database client for the current library.
	pub db: Arc<PrismaClient>,
	pub sync: Arc<SyncManager>,
	/// key manager that provides encryption keys to functions that require them
	// pub key_manager: Arc<KeyManager>,
	/// node_local_id holds the local ID of the node which is running the library.
	pub node_local_id: i32,
	/// node_context holds the node context for the node which this library is running on.
	pub(super) node_context: NodeContext,
	/// p2p identity
	pub identity: Arc<Identity>,
	pub orphan_remover: OrphanRemoverActor,
}

impl Debug for Library {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// Rolling out this implementation because `NodeContext` contains a DynJob which is
		// troublesome to implement Debug trait
		f.debug_struct("LibraryContext")
			.field("id", &self.id)
			.field("config", &self.config)
			.field("db", &self.db)
			.field("node_local_id", &self.node_local_id)
			.finish()
	}
}

impl Library {
	pub(crate) async fn spawn_job<SJob, Init>(
		&self,
		jobable: impl IntoJob<SJob>,
	) -> Result<(), JobManagerError>
	where
		SJob: StatefulJob<Init = Init> + 'static,
		Init: JobInitData + 'static,
	{
		self.node_context
			.jobs
			.clone()
			.ingest(self, jobable.into_job())
			.await
	}

	pub(crate) fn emit(&self, event: CoreEvent) {
		if let Err(e) = self.node_context.event_bus_tx.send(event) {
			warn!("Error sending event to event bus: {e:?}");
		}
	}

	pub(crate) fn config(&self) -> Arc<NodeConfigManager> {
		self.node_context.config.clone()
	}

	pub(crate) fn location_manager(&self) -> &Arc<LocationManager> {
		&self.node_context.location_manager
	}

	pub async fn thumbnail_exists(&self, cas_id: &str) -> Result<bool, FileIOError> {
		let thumb_path = get_thumbnail_path(self, cas_id);

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
					file_path::location::is(vec![location::node_id::equals(Some(
						self.node_local_id,
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
}
