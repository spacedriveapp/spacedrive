#![warn(clippy::unwrap_used, clippy::panic)]

use crate::{
	api::{CoreEvent, Router},
	job::JobManager,
	library::LibraryManager,
	location::{LocationManager, LocationManagerError},
	node::NodeConfigManager,
	p2p::{sync::NetworkedLibraryManager, P2PManager},
};

use api::notifications::{Notification, NotificationData, NotificationId};
use chrono::{DateTime, Utc};
use sd_p2p::trustedhosts::{TrustedHostError, TrustedHostRegistry};
pub use sd_prisma::*;

use std::{
	ops::Deref,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicU32, Ordering},
		Arc,
	},
};

use thiserror::Error;
use tokio::{fs, sync::broadcast};
use tracing::{error, info, warn};
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

/// Holds references to all the services that make up the Spacedrive core.
/// This can easily be passed around as a context to the rest of the core.
pub struct NodeServices {
	pub config: Arc<NodeConfigManager>,
	pub job_manager: Arc<JobManager>,
	pub location_manager: LocationManager,
	pub p2p: Arc<P2PManager>,
	pub event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	pub notifications: NotificationManager,
	pub nlm: Arc<NetworkedLibraryManager>,
}

/// Represents a single running instance of the Spacedrive core.
pub struct Node {
	pub data_dir: PathBuf,
	pub library_manager: Arc<LibraryManager>,
}

// This isn't idiomatic but it will work for now
impl Deref for Node {
	type Target = NodeServices;

	fn deref(&self) -> &Self::Target {
		&self.library_manager.node
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
		let config = NodeConfigManager::new(data_dir.to_path_buf())
			.await
			.map_err(NodeError::FailedToInitializeConfig)?;

		let (p2p, p2p_stream) = P2PManager::new(config.clone()).await?;

		let services = Arc::new(NodeServices {
			job_manager: JobManager::new(),
			location_manager: LocationManager::new(),
			nlm: NetworkedLibraryManager::new(p2p.clone()),
			notifications: NotificationManager::new(),
			p2p,
			config,
			event_bus,
		});

		let node = Arc::new(Node {
			data_dir: data_dir.to_path_buf(),
			library_manager: LibraryManager::new(data_dir.join("libraries"), services).await?,
		});

		#[cfg(debug_assertions)]
		if let Some(init_data) = init_data {
			init_data
				.apply(&node.library_manager, node.config.get().await)
				.await?;
		}

		node.p2p
			.start(p2p_stream, node.library_manager.clone(), node.nlm.clone());

		let router = api::mount();
		info!("Spacedrive online.");

		Ok((node, router))
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
	pub fn new() -> Self {
		let (tx, _) = broadcast::channel(30);
		Self(tx, AtomicU32::new(0))
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
	#[error("failed to initialize trusted hosts: {0}")]
	TrustedHost(#[from] TrustedHostError),
}
