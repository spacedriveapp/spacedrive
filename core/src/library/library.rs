use crate::{
	api::{
		notifications::{Notification, NotificationData, NotificationId},
		CoreEvent,
	},
	location::file_path_helper::{file_path_to_full_path, IsolatedFilePathData},
	notifications,
	object::{media::thumbnail::get_indexed_thumbnail_path, orphan_remover::OrphanRemoverActor},
	prisma::{file_path, location, PrismaClient},
	sync,
	util::{db::maybe_missing, error::FileIOError},
	Node,
};

use futures::{Future, SinkExt};
use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::notification;
use tracing_subscriber::{layer::SubscriberExt, Layer};

use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	io::Write,
	path::{Path, PathBuf},
	pin::Pin,
	sync::Arc,
};

use chrono::{DateTime, Utc};
use tokio::{
	fs, io,
	sync::{broadcast, Mutex},
	sync::{oneshot, RwLock},
	task::AbortHandle,
};
use tracing::{instrument::WithSubscriber, level_filters::LevelFilter, warn, Subscriber};
use uuid::Uuid;

use super::{LibraryConfig, LibraryManagerError};

// TODO: Finish this
// pub enum LibraryNew {
// 	InitialSync,
// 	Encrypted,
// 	Loaded(LoadedLibrary),
//  Deleting,
// }

struct Actor {
	pub abort_handle: Mutex<Option<AbortHandle>>,
	pub spawn_fn: Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>,
}

pub struct Actors {
	pub invalidate_rx: broadcast::Receiver<()>,
	invalidate_tx: broadcast::Sender<()>,
	actors: Arc<Mutex<HashMap<String, Arc<Actor>>>>,
}

impl Actors {
	pub async fn declare<F: Future<Output = ()> + Send + 'static>(
		self: &Arc<Self>,
		name: &str,
		actor_fn: impl FnOnce() -> F + Send + Sync + Clone + 'static,
		autostart: bool,
	) {
		let mut actors = self.actors.lock().await;

		actors.insert(
			name.to_string(),
			Arc::new(Actor {
				abort_handle: Default::default(),
				spawn_fn: Arc::new(move || Box::pin((actor_fn.clone())()) as Pin<Box<_>>),
			}),
		);

		if autostart {
			self.start(name).await;
		}
	}

	pub async fn start(self: &Arc<Self>, name: &str) {
		let name = name.to_string();
		let actors = self.actors.lock().await;

		let Some(actor) = actors.get(&name).cloned() else {
			return;
		};

		let mut abort_handle = actor.abort_handle.lock().await;
		if abort_handle.is_some() {
			return;
		}

		let (tx, rx) = oneshot::channel();

		let invalidate_tx = self.invalidate_tx.clone();

		let spawn_fn = actor.spawn_fn.clone();

		tokio::spawn(async move {
			let mut subscriber_rx = subscriber_rx;
			while let Some(r) = subscriber_rx.recv().await {
				println!("{}", String::from_utf8(r).unwrap());
			}
		});

		let task = tokio::spawn(async move {
			(spawn_fn)().await;

			tx.send(()).ok();
		});

		*abort_handle = Some(task.abort_handle());
		invalidate_tx.send(()).ok();

		tokio::spawn({
			let actor = actor.clone();
			async move {
				match rx.await {
					_ => {}
				};

				actor.abort_handle.lock().await.take();
				invalidate_tx.send(()).ok();
			}
		});
	}

	pub async fn stop(self: &Arc<Self>, name: &str) {
		let name = name.to_string();
		let actors = self.actors.lock().await;

		let Some(actor) = actors.get(&name).cloned() else {
			return;
		};

		let mut abort_handle = actor.abort_handle.lock().await;

		if let Some(abort_handle) = abort_handle.take() {
			abort_handle.abort();
		}
	}

	pub async fn get_state(&self) -> HashMap<String, bool> {
		let actors = self.actors.lock().await;

		let mut state = HashMap::new();

		for (name, actor) in &*actors {
			state.insert(name.to_string(), actor.abort_handle.lock().await.is_some());
		}

		state
	}
}

impl Default for Actors {
	fn default() -> Self {
		let actors = Default::default();

		let (invalidate_tx, invalidate_rx) = broadcast::channel(1);

		Self {
			actors,
			invalidate_rx,
			invalidate_tx,
		}
	}
}

pub struct Library {
	/// id holds the ID of the current library.
	pub id: Uuid,
	/// config holds the configuration of the current library.
	/// KEEP PRIVATE: Access through `Self::config` method.
	config: RwLock<LibraryConfig>,
	/// db holds the database client for the current library.
	pub db: Arc<PrismaClient>,
	pub sync: Arc<sync::Manager>,
	/// key manager that provides encryption keys to functions that require them
	// pub key_manager: Arc<KeyManager>,
	/// p2p identity
	pub identity: Arc<Identity>,
	pub orphan_remover: OrphanRemoverActor,
	// The UUID which matches `config.instance_id`'s primary key.
	pub instance_uuid: Uuid,

	notifications: notifications::Notifications,
	pub env: Arc<crate::env::Env>,

	// Look, I think this shouldn't be here but our current invalidation system needs it.
	// TODO(@Oscar): Get rid of this with the new invalidation system.
	event_bus_tx: broadcast::Sender<CoreEvent>,

	pub actors: Arc<Actors>,
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
	pub async fn new(
		id: Uuid,
		config: LibraryConfig,
		instance_uuid: Uuid,
		identity: Arc<Identity>,
		db: Arc<PrismaClient>,
		node: &Arc<Node>,
		sync: Arc<sync::Manager>,
	) -> Arc<Self> {
		Arc::new(Self {
			id,
			config: RwLock::new(config),
			sync,
			db: db.clone(),
			// key_manager,
			identity,
			orphan_remover: OrphanRemoverActor::spawn(db),
			notifications: node.notifications.clone(),
			instance_uuid,
			env: node.env.clone(),
			event_bus_tx: node.event_bus.0.clone(),
			actors: Default::default(),
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
			warn!("Error sending event to event bus: {e:?}");
		}
	}

	pub async fn thumbnail_exists(&self, node: &Node, cas_id: &str) -> Result<bool, FileIOError> {
		let thumb_path = get_indexed_thumbnail_path(node, cas_id, self.id);

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
					.unwrap_or_default(),
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

		self.notifications._internal_send(Notification {
			id: NotificationId::Library(self.id, result.id as u32),
			data,
			read: false,
			expires,
		});
	}
}
