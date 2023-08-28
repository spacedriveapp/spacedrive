#![warn(clippy::unwrap_used, clippy::panic)]

use crate::{
	api::{CoreEvent, Router},
	location::LocationManagerError,
	object::thumbnail_remover,
	p2p::sync::NetworkedLibraries,
};

use api::notifications::{Notification, NotificationData, NotificationId};
use chrono::{DateTime, Utc};
use node::config;
use notifications::Notifications;
pub use sd_prisma::*;

use std::{
	env, fmt,
	path::{Path, PathBuf},
	sync::Arc,
};

use thiserror::Error;
use tokio::{fs, sync::broadcast};
use tracing::{error, info, warn};
use tracing_appender::{
	non_blocking::{NonBlocking, WorkerGuard},
	rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
	filter::{Directive, FromEnvError, LevelFilter},
	fmt as tracing_fmt,
	prelude::*,
	EnvFilter,
};

pub mod api;
pub mod custom_uri;
pub(crate) mod job;
pub mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod notifications;
pub(crate) mod object;
pub(crate) mod p2p;
pub(crate) mod preferences;
#[doc(hidden)] // TODO(@Oscar): Make this private when breaking out `utils` into `sd-utils`
pub mod util;
pub(crate) mod volume;

pub(crate) use sd_core_sync as sync;

/// Represents a single running instance of the Spacedrive core.
/// Holds references to all the services that make up the Spacedrive core.
pub struct Node {
	pub data_dir: PathBuf,
	pub config: Arc<config::Manager>,
	pub libraries: Arc<library::Libraries>,
	pub jobs: Arc<job::Jobs>,
	pub locations: location::Locations,
	pub p2p: Arc<p2p::P2PManager>,
	pub event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	pub notifications: Notifications,
	pub nlm: Arc<NetworkedLibraries>,
	pub thumbnail_remover: thumbnail_remover::Actor,
}

impl fmt::Debug for Node {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Node")
			.field("data_dir", &self.data_dir)
			.finish()
	}
}

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
		let data_dir = data_dir.as_ref();

		info!("Starting core with data directory '{}'", data_dir.display());

		#[cfg(debug_assertions)]
		let init_data = util::debug_initializer::InitConfig::load(data_dir).await?;

		// This error is ignored because it's throwing on mobile despite the folder existing.
		let _ = fs::create_dir_all(&data_dir).await;

		let event_bus = broadcast::channel(1024);
		let config = config::Manager::new(data_dir.to_path_buf())
			.await
			.map_err(NodeError::FailedToInitializeConfig)?;

		let (p2p, p2p_stream) = p2p::P2PManager::new(config.clone()).await?;

		let (locations, locations_actor) = location::Locations::new();
		let (jobs, jobs_actor) = job::Jobs::new();
		let libraries = library::Libraries::new(data_dir.join("libraries")).await?;
		let node = Arc::new(Node {
			data_dir: data_dir.to_path_buf(),
			jobs,
			locations,
			nlm: NetworkedLibraries::new(p2p.clone(), &libraries),
			notifications: notifications::Notifications::new(),
			p2p,
			config,
			event_bus,
			thumbnail_remover: thumbnail_remover::Actor::new(
				data_dir.to_path_buf(),
				libraries.clone(),
			),
			libraries,
		});

		// Setup start actors that depend on the `Node`
		#[cfg(debug_assertions)]
		if let Some(init_data) = init_data {
			init_data.apply(&node.libraries, &node).await?;
		}

		// It's import these are run after libraries are loaded!
		locations_actor.start(node.clone());

		// Finally load the libraries from disk into the library manager
		node.libraries.init(&node).await?;

		jobs_actor.start(node.clone());
		node.p2p.start(p2p_stream, node.clone());

		let router = api::mount();
		info!("Spacedrive online.");
		Ok((node, router))
	}

	pub fn init_logger(data_dir: impl AsRef<Path>) -> Result<WorkerGuard, FromEnvError> {
		let (logfile, guard) = NonBlocking::new(
			RollingFileAppender::builder()
				.filename_prefix("sd.log")
				.rotation(Rotation::DAILY)
				.max_log_files(4)
				.build(data_dir.as_ref().join("logs"))
				.expect("Error setting up log file!"),
		);

		// Set a default if the user hasn't set an override
		if env::var("RUST_LOG") == Err(env::VarError::NotPresent) {
			let directive: Directive = if cfg!(debug_assertions) {
				LevelFilter::DEBUG
			} else {
				LevelFilter::INFO
			}
			.into();
			env::set_var("RUST_LOG", directive.to_string());
		}

		let collector = tracing_subscriber::registry()
			.with(
				tracing_fmt::Subscriber::new()
					.with_ansi(false)
					.with_writer(logfile),
			)
			.with(
				tracing_fmt::Subscriber::new()
					.with_writer(std::io::stdout)
					.with_filter(
						EnvFilter::builder()
							.from_env()?
							// We don't wanna blow up the logs
							.add_directive("sd_core::location::manager=info".parse()?),
					),
			);

		tracing::collect::set_global_default(collector)
			.map_err(|err| {
				eprintln!("Error initializing global logger: {:?}", err);
			})
			.ok();

		std::panic::set_hook(Box::new(move |panic| {
			if let Some(location) = panic.location() {
				tracing::error!(
					message = %panic,
					panic.file = format!("{}:{}", location.file(), location.line()),
					panic.column = location.column(),
				);
			} else {
				tracing::error!(message = %panic);
			}
		}));

		Ok(guard)
	}

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.jobs.shutdown().await;
		self.p2p.shutdown().await;
		info!("Spacedrive Core shutdown successful!");
	}

	pub(crate) fn emit(&self, event: CoreEvent) {
		if let Err(e) = self.event_bus.0.send(event) {
			warn!("Error sending event to event bus: {e:?}");
		}
	}

	pub async fn emit_notification(&self, data: NotificationData, expires: Option<DateTime<Utc>>) {
		let notification = Notification {
			id: NotificationId::Node(self.notifications._internal_next_id()),
			data,
			read: false,
			expires,
		};

		match self
			.config
			.write(|mut cfg| cfg.notifications.push(notification.clone()))
			.await
		{
			Ok(_) => {
				self.notifications._internal_send(notification);
			}
			Err(err) => {
				error!("Error saving notification to config: {:?}", err);
			}
		}
	}
}

/// Error type for Node related errors.
#[derive(Error, Debug)]
pub enum NodeError {
	#[error("NodeError::FailedToInitializeConfig({0})")]
	FailedToInitializeConfig(util::migrator::MigratorError),
	#[error("failed to initialize library manager: {0}")]
	FailedToInitializeLibraryManager(#[from] library::LibraryManagerError),
	#[error("failed to initialize location manager: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("failed to initialize p2p manager: {0}")]
	P2PManager(#[from] sd_p2p::ManagerError),
	#[error("invalid platform integer: {0}")]
	InvalidPlatformInt(u8),
	#[cfg(debug_assertions)]
	#[error("Init config error: {0}")]
	InitConfig(#[from] util::debug_initializer::InitConfigError),
	#[error("logger error: {0}")]
	Logger(#[from] FromEnvError),
}
