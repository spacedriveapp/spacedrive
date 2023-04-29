use crate::{
	api::{CoreEvent, Router},
	job::JobManager,
	library::LibraryManager,
	location::{LocationManager, LocationManagerError},
	node::NodeConfigManager,
	p2p::P2PManager,
};

use std::{path::Path, sync::Arc};
use thiserror::Error;
use tokio::{fs, sync::broadcast};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{prelude::*, EnvFilter};

pub mod api;
pub mod custom_uri;
pub(crate) mod job;
pub mod library;
pub(crate) mod location;
pub(crate) mod migrations;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod p2p;
pub(crate) mod sync;
pub(crate) mod util;
pub(crate) mod volume;

#[allow(warnings, unused)]
mod prisma;
pub(crate) mod prisma_sync;

#[derive(Clone)]
pub struct NodeContext {
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
	pub location_manager: Arc<LocationManager>,
	pub event_bus_tx: broadcast::Sender<CoreEvent>,
	pub p2p: Arc<P2PManager>,
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	pub library_manager: Arc<LibraryManager>,
	location_manager: Arc<LocationManager>,
	jobs: Arc<JobManager>,
	p2p: Arc<P2PManager>,
	event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	// peer_request: tokio::sync::Mutex<Option<PeerRequest>>,
}

#[cfg(not(target_os = "android"))]
const CONSOLE_LOG_FILTER: tracing_subscriber::filter::LevelFilter = {
	use tracing_subscriber::filter::LevelFilter;

	match cfg!(debug_assertions) {
		true => LevelFilter::DEBUG,
		false => LevelFilter::INFO,
	}
};

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
		let data_dir = data_dir.as_ref();

		#[cfg(debug_assertions)]
		let init_data = util::debug_initializer::InitConfig::load(data_dir).await;

		// This error is ignored because it's throwing on mobile despite the folder existing.
		let _ = fs::create_dir_all(&data_dir).await;

		// dbg!(get_object_kind_from_extension("png"));

		// let (non_blocking, _guard) = tracing_appender::non_blocking(rolling::daily(
		// 	Path::new(&data_dir).join("logs"),
		// 	"log",
		// ));
		// TODO: Make logs automatically delete after x time https://github.com/tokio-rs/tracing/pull/2169

		let subscriber = tracing_subscriber::registry().with(
			EnvFilter::from_default_env()
				.add_directive("warn".parse().expect("Error invalid tracing directive!"))
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
					"desktop=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				),
			// .add_directive(
			// 	"rspc=debug"
			// 		.parse()
			// 		.expect("Error invalid tracing directive!"),
			// ),
		);
		#[cfg(not(target_os = "android"))]
		let subscriber = subscriber.with(tracing_subscriber::fmt::layer().with_filter(CONSOLE_LOG_FILTER));
		// #[cfg(target_os = "android")]
		// let subscriber = subscriber.with(tracing_android::layer("com.spacedrive.app").unwrap()); // TODO: This is not working
		subscriber
			// .with(
			// 	Layer::default()
			// 		.with_writer(non_blocking)
			// 		.with_ansi(false)
			// 		.with_filter(LevelFilter::DEBUG),
			// )
			.init();

		let event_bus = broadcast::channel(1024);
		let config = NodeConfigManager::new(data_dir.to_path_buf()).await?;

		let jobs = JobManager::new();
		let location_manager = LocationManager::new();
		let (p2p, mut p2p_rx) = P2PManager::new(config.clone()).await;

		let library_manager = LibraryManager::new(
			data_dir.join("libraries"),
			NodeContext {
				config: config.clone(),
				jobs: jobs.clone(),
				location_manager: location_manager.clone(),
				p2p: p2p.clone(),
				event_bus_tx: event_bus.0.clone(),
			},
		)
		.await?;

		#[cfg(debug_assertions)]
		if let Some(init_data) = init_data {
			init_data.apply(&library_manager).await;
		}

		debug!("Watching locations");

		tokio::spawn({
			let library_manager = library_manager.clone();

			async move {
				while let Ok((library_id, operations)) = p2p_rx.recv().await {
					debug!("going to ingest {} operations", operations.len());

					let Some(library) = library_manager.get_library(library_id).await else {
						warn!("no library found!");
						continue;
					};

					for op in operations {
						println!("ingest lib id: {}", library.id);
						library.sync.ingest_op(op).await.unwrap();
					}
				}
			}
		});

		let router = api::mount();
		let node = Node {
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

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.jobs.pause().await;
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
	#[error("Failed to create data directory: {0}")]
	FailedToCreateDataDirectory(#[from] std::io::Error),
	#[error("Failed to initialize config: {0}")]
	FailedToInitializeConfig(#[from] util::migrator::MigratorError),
	#[error("Failed to initialize library manager: {0}")]
	FailedToInitializeLibraryManager(#[from] library::LibraryManagerError),
	#[error("Location manager error: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("invalid platform integer")]
	InvalidPlatformInt(i32),
}
