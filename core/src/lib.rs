#![warn(clippy::unwrap_used, clippy::panic)]

use crate::{
	api::{CoreEvent, Router},
	job::JobManager,
	library::LibraryManager,
	location::{LocationManager, LocationManagerError},
	node::NodeConfigManager,
	p2p::P2PManager,
};

use api::notifications::{Notification, NotificationData, NotificationId};
use chrono::{DateTime, Utc};
pub use sd_prisma::*;

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicU32, Ordering},
		Arc,
	},
};

use thiserror::Error;
use tokio::{fs, sync::broadcast};
use tracing::{debug, error, info, warn};
use tracing_appender::{
	non_blocking::{NonBlocking, WorkerGuard},
	rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub mod api;
pub mod custom_uri;
pub(crate) mod job;
pub mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod p2p;
pub(crate) mod preferences;
pub(crate) mod util;
pub(crate) mod volume;

#[derive(Clone)]
pub struct NodeContext {
	pub config: Arc<NodeConfigManager>,
	pub job_manager: Arc<JobManager>,
	pub location_manager: Arc<LocationManager>,
	pub p2p: Arc<P2PManager>,
	pub event_bus_tx: broadcast::Sender<CoreEvent>,
	pub notifications: Arc<NotificationManager>,
}

pub struct Node {
	pub data_dir: PathBuf,
	config: Arc<NodeConfigManager>,
	pub library_manager: Arc<LibraryManager>,
	location_manager: Arc<LocationManager>,
	job_manager: Arc<JobManager>,
	p2p: Arc<P2PManager>,
	event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	notifications: Arc<NotificationManager>,
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
		let config = NodeConfigManager::new(data_dir.to_path_buf())
			.await
			.map_err(NodeError::FailedToInitializeConfig)?;
		debug!("Initialised 'NodeConfigManager'...");

		let job_manager = JobManager::new();
		debug!("Initialised 'JobManager'...");

		let notifications = NotificationManager::new();
		debug!("Initialised 'NotificationManager'...");

		let location_manager = LocationManager::new();
		debug!("Initialised 'LocationManager'...");

		let (p2p, p2p_stream) = P2PManager::new(config.clone()).await?;
		debug!("Initialised 'P2PManager'...");

		let library_manager = LibraryManager::new(
			data_dir.join("libraries"),
			NodeContext {
				config: config.clone(),
				job_manager: job_manager.clone(),
				location_manager: location_manager.clone(),
				p2p: p2p.clone(),
				event_bus_tx: event_bus.0.clone(),
				notifications: notifications.clone(),
			},
		)
		.await?;
		debug!("Initialised 'LibraryManager'...");

		p2p.start(p2p_stream, library_manager.clone());

		#[cfg(debug_assertions)]
		if let Some(init_data) = init_data {
			init_data
				.apply(&library_manager, config.get().await)
				.await?;
		}

		let router = api::mount();
		let node = Node {
			data_dir: data_dir.to_path_buf(),
			config,
			library_manager,
			location_manager,
			job_manager,
			p2p,
			event_bus,
			notifications,
		};

		info!("Spacedrive online.");
		Ok((Arc::new(node), router))
	}

	pub fn init_logger(data_dir: impl AsRef<Path>) -> WorkerGuard {
		let (logfile, guard) = NonBlocking::new(
			RollingFileAppender::builder()
				.filename_prefix("sd.log")
				.rotation(Rotation::DAILY)
				.max_log_files(4)
				.build(data_dir.as_ref().join("logs"))
				.expect("Error setting up log file!"),
		);

		let collector = tracing_subscriber::registry()
			.with(fmt::Subscriber::new().with_ansi(false).with_writer(logfile))
			.with(
				fmt::Subscriber::new()
					.with_writer(std::io::stdout)
					.with_filter(if cfg!(debug_assertions) {
						EnvFilter::from_default_env()
							.add_directive(
								"warn".parse().expect("Error invalid tracing directive!"),
							)
							.add_directive(
								"sd_core=debug"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
							.add_directive(
								"sd_core::location::manager=info"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
							.add_directive(
								"sd_core_mobile=debug"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
							// .add_directive(
							// 	"sd_p2p=debug"
							// 		.parse()
							// 		.expect("Error invalid tracing directive!"),
							// )
							.add_directive(
								"server=debug"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
							.add_directive(
								"spacedrive=debug"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
							.add_directive(
								"rspc=debug"
									.parse()
									.expect("Error invalid tracing directive!"),
							)
					} else {
						EnvFilter::from("info")
					}),
			);

		tracing::collect::set_global_default(collector)
			.map_err(|err| {
				eprintln!("Error initializing global logger: {:?}", err);
			})
			.ok();

		let prev_hook = std::panic::take_hook();
		std::panic::set_hook(Box::new(move |panic_info| {
			error!("{}", panic_info);
			prev_hook(panic_info);
		}));

		guard
	}

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.job_manager.shutdown().await;
		self.p2p.shutdown().await;
		info!("Spacedrive Core shutdown successful!");
	}

	pub async fn emit_notification(&self, data: NotificationData, expires: Option<DateTime<Utc>>) {
		let notification = Notification {
			id: NotificationId::Node(self.notifications.1.fetch_add(1, Ordering::SeqCst)),
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
				self.notifications.0.send(notification).ok();
			}
			Err(err) => {
				error!("Error saving notification to config: {:?}", err);
			}
		}
	}
}

pub struct NotificationManager(
	// Keep this private and use `Node::emit_notification` or `Library::emit_notification` instead.
	broadcast::Sender<Notification>,
	// Counter for `NotificationId::Node(_)`. NotificationId::Library(_, _)` is autogenerated by the DB.
	AtomicU32,
);

impl NotificationManager {
	pub fn new() -> Arc<Self> {
		let (tx, _) = broadcast::channel(30);
		Arc::new(Self(tx, AtomicU32::new(0)))
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
		self.0.subscribe()
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
}
