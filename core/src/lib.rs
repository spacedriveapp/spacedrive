use api::{CoreEvent, Ctx, Router};
use job::JobManager;
use library::LibraryManager;
use node::NodeConfigManager;
use std::{path::Path, sync::Arc};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

use tokio::{fs, sync::broadcast};

pub use rspc; // We expose rspc so we can access it in the Desktop app

pub(crate) mod api;
pub(crate) mod encode;
pub(crate) mod file;
pub(crate) mod job;
pub(crate) mod library;
pub(crate) mod node;
pub(crate) mod prisma;
pub(crate) mod sys;
pub(crate) mod tag;
pub(crate) mod util;

#[derive(Clone)]
pub struct NodeContext {
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
	pub event_bus_tx: broadcast::Sender<CoreEvent>,
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	library_manager: Arc<LibraryManager>,
	jobs: Arc<JobManager>,
	event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
}

#[cfg(debug_assertions)]
const CONSOLE_LOG_FILTER: LevelFilter = LevelFilter::DEBUG;

#[cfg(not(debug_assertions))]
const CONSOLE_LOG_FILTER: LevelFilter = LevelFilter::INFO;

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> (Arc<Node>, Arc<Router>) {
		fs::create_dir_all(&data_dir).await.unwrap();

		tracing_subscriber::registry()
			.with(
				EnvFilter::from_default_env()
					.add_directive("warn".parse().expect("Error invalid tracing directive!"))
					.add_directive(
						"sdcore=debug"
							.parse()
							.expect("Error invalid tracing directive!"),
					)
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
			)
			.with(fmt::layer().with_filter(CONSOLE_LOG_FILTER))
			.init();

		let event_bus = broadcast::channel(1024);
		let config = NodeConfigManager::new(data_dir.as_ref().to_owned())
			.await
			.unwrap();
		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			config: config.clone(),
			jobs: jobs.clone(),
			event_bus_tx: event_bus.0.clone(),
		};

		let router = api::mount();
		let node = Node {
			config,
			library_manager: LibraryManager::new(data_dir.as_ref().join("libraries"), node_ctx)
				.await
				.unwrap(),
			jobs,
			event_bus,
		};

		(Arc::new(node), router)
	}

	pub fn get_request_context(&self) -> Ctx {
		Ctx {
			library_manager: Arc::clone(&self.library_manager),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
			event_bus: self.event_bus.0.clone(),
		}
	}
}
