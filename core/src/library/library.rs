use crate::{
	api::{
		notifications::{Notification, NotificationData, NotificationId},
		CoreEvent,
	},
	location::{
		file_path_helper::{file_path_to_full_path, IsolatedFilePathData},
		LocationManager,
	},
	node::NodeConfigManager,
	object::{orphan_remover::OrphanRemoverActor, preview::get_thumbnail_path, thumbnail_remover::ThumbnailRemoverActor},
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

use chrono::{DateTime, Utc};
use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::notification;
use tokio::{fs, io};
use tracing::warn;
use uuid::Uuid;

use super::{LibraryConfig, LibraryManagerError};

/// LibraryContext holds context for a library which can be passed around the application.
#[derive(Clone)]
pub struct Library {
	/// id holds the ID of the current library.
	pub id: Uuid,
	/// config holds the configuration of the current library.
	pub config: LibraryConfig,
	/// db holds the database client for the current library.
	pub db: Arc<PrismaClient>,
	pub sync: Arc<SyncManager>,
	/// key manager that provides encryption keys to functions that require them
	// pub key_manager: Arc<KeyManager>,
	/// node_context holds the node context for the node which this library is running on.
	pub node_context: NodeContext,
	/// p2p identity
	pub identity: Arc<Identity>,
	pub orphan_remover: OrphanRemoverActor,
	pub thumbnail_remover: ThumbnailRemoverActor,
}

impl Debug for Library {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// Rolling out this implementation because `NodeContext` contains a DynJob which is
		// troublesome to implement Debug trait
		f.debug_struct("LibraryContext")
			.field("id", &self.id)
			.field("config", &self.config)
			.field("db", &self.db)
			.finish()
	}
}

impl Library {
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
					// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
					file_path::location::is(vec![location::instance_id::equals(Some(
						self.config.instance_id,
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

	/// Create a new notification which will be stored into the DB and emitted to the UI.
	pub async fn emit_notification(&self, data: NotificationData, expires: Option<DateTime<Utc>>) {
		let result = match self
			.db
			.notification()
			.create(
				match rmp_serde::to_vec(&data).map_err(|err| err.to_string()) {
					Ok(data) => data,
					Err(err) => {
						warn!(
							"Failed to serialize notification data for library '{}': {}",
							self.id, err
						);
						return;
					}
				},
				expires
					.map(|e| vec![notification::expires_at::set(Some(e.fixed_offset()))])
					.unwrap_or_else(Vec::new),
			)
			.exec()
			.await
		{
			Ok(result) => result,
			Err(err) => {
				warn!(
					"Failed to create notification in library '{}': {}",
					self.id, err
				);
				return;
			}
		};

		self.node_context
			.notifications
			.0
			.send(Notification {
				id: NotificationId::Library(self.id, result.id as u32),
				data,
				read: false,
				expires,
			})
			.ok();
	}
}
