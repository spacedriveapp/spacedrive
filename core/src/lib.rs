use api::{Ctx, Router};
use job::JobManager;
use library::LibraryManager;
use node::NodeConfigManager;
use std::{path::Path, sync::Arc};

use tokio::fs;

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
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	library_manager: Arc<LibraryManager>,
	jobs: Arc<JobManager>,
}

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> (Arc<Node>, Arc<Router>) {
		// TODO: Move to tokio::tracing
		// dotenv().ok();
		// env_logger::init();

		fs::create_dir_all(&data_dir).await.unwrap();

		let config = NodeConfigManager::new(data_dir.as_ref().to_owned())
			.await
			.unwrap();
		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			config: config.clone(),
			jobs: jobs.clone(),
		};

		let router = api::mount();
		let node = Node {
			config,
			library_manager: LibraryManager::new(data_dir.as_ref().join("libraries"), node_ctx)
				.await
				.unwrap(),
			jobs,
		};

		(Arc::new(node), router)
	}

	pub fn get_request_context(&self, library_id: Option<String>) -> Ctx {
		Ctx {
			library_id,
			library_manager: Arc::clone(&self.library_manager),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
		}
	}
}
