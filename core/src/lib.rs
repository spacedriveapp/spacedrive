#![warn(clippy::unwrap_used, clippy::panic)]

use crate::{
	api::{CoreEvent, Router},
	job::JobManager,
	library::LibraryManager,
	location::{LocationManager, LocationManagerError},
	node::NodeConfigManager,
	p2p::P2PManager,
};

pub use sd_prisma::*;

use std::{
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
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub mod api;
pub mod custom_uri;
pub(crate) mod job;
pub mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod p2p;
pub(crate) mod sync;
pub(crate) mod util;
pub(crate) mod volume;

#[derive(Clone)]
pub struct NodeContext {
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
	pub location_manager: Arc<LocationManager>,
	pub event_bus_tx: broadcast::Sender<CoreEvent>,
}

pub struct Node {
	pub data_dir: PathBuf,
	config: Arc<NodeConfigManager>,
	pub library_manager: Arc<LibraryManager>,
	location_manager: Arc<LocationManager>,
	jobs: Arc<JobManager>,
	p2p: Arc<P2PManager>,
	event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	// peer_request: tokio::sync::Mutex<Option<PeerRequest>>,
}

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
		let data_dir = data_dir.as_ref();

		#[cfg(debug_assertions)]
		let init_data = util::debug_initializer::InitConfig::load(data_dir).await?;

		// This error is ignored because it's throwing on mobile despite the folder existing.
		let _ = fs::create_dir_all(&data_dir).await;

		let event_bus = broadcast::channel(1024);
		let config = NodeConfigManager::new(data_dir.to_path_buf())
			.await
			.map_err(NodeError::FailedToInitializeConfig)?;

		let jobs = JobManager::new();
		let location_manager = LocationManager::new();
		let library_manager = LibraryManager::new(
			data_dir.join("libraries"),
			NodeContext {
				config: config.clone(),
				jobs: jobs.clone(),
				location_manager: location_manager.clone(),
				// p2p: p2p.clone(),
				event_bus_tx: event_bus.0.clone(),
			},
		)
		.await?;
		let p2p = P2PManager::new(config.clone(), library_manager.clone()).await?;

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
			jobs,
			p2p,
			event_bus,
			// peer_request: tokio::sync::Mutex::new(None),
		};

		info!("Spacedrive online.");
		Ok((Arc::new(node), router))
	}

	pub fn init_logger(data_dir: impl AsRef<Path>) -> WorkerGuard {
		let log_filter = match cfg!(debug_assertions) {
			true => tracing::Level::DEBUG,
			false => tracing::Level::INFO,
		};

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
					.with_writer(std::io::stdout.with_max_level(log_filter))
					.with_filter(
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
							),
					),
			);

		tracing::collect::set_global_default(collector)
			.map_err(|err| {
				println!("Error initializing global logger: {:?}", err);
			})
			.ok();

		guard
	}

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.jobs.clone().shutdown().await;
		self.p2p.shutdown().await;
		info!("Spacedrive Core shutdown successful!");
	}

	// pub async fn begin_guest_peer_request(
	// 	&self,
	// 	peer_id: String,
	// ) -> Option<Receiver<peer_request::guest::State>> {
	// 	let mut pr_guard = self.peer_request.lock().await;

	// 	if pr_guard.is_some() {
	// 		return None;
	// 	}

	// 	let (req, stream) = peer_request::guest::PeerRequest::new_actor(peer_id);
	// 	*pr_guard = Some(PeerRequest::Guest(req));
	// 	Some(stream)
	// }
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
