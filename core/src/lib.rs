use api::{CoreEvent, Ctx, Router};
use futures::executor::block_on;
use job::JobManager;
use library::LibraryManager;
use node::NodeConfigManager;
use std::{fs::File, io::Read, path::Path, sync::Arc};
use tracing::{error, info};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

use tokio::{fs, sync::broadcast};

pub mod api;
pub(crate) mod encode;
pub(crate) mod file;
pub(crate) mod job;
pub(crate) mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod prisma;
pub(crate) mod util;
pub(crate) mod volume;

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
		let data_dir = data_dir.as_ref();
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
		let config = NodeConfigManager::new(data_dir.to_owned()).await.unwrap();

		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			config: config.clone(),
			jobs: jobs.clone(),
			event_bus_tx: event_bus.0.clone(),
		};
		let library_manager =
			LibraryManager::new(data_dir.to_owned().join("libraries"), node_ctx.clone())
				.await
				.unwrap();

		// Trying to resume possible paused jobs
		let inner_library_manager = Arc::clone(&library_manager);
		let inner_jobs = Arc::clone(&jobs);
		tokio::spawn(async move {
			for library_ctx in inner_library_manager.get_all_libraries_ctx().await {
				if let Err(e) = Arc::clone(&inner_jobs).resume_jobs(&library_ctx).await {
					error!("Failed to resume jobs for library. {:#?}", e);
				}
			}
		});

		let router = api::mount();
		let node = Node {
			config,
			library_manager: LibraryManager::new(data_dir.join("libraries"), node_ctx)
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

	// Note: this system doesn't use chunked encoding which could prove a problem with large files but I can't see an easy way to do chunked encoding with Tauri custom URIs.
	// It would also be nice to use Tokio Filesystem operations instead of the std ones which block. Tauri's custom URI protocols don't seem to support async out of the box.
	pub fn handle_custom_uri(
		&self,
		path: Vec<&str>,
	) -> (
		u16,     /* Status Code */
		&str,    /* Content-Type */
		Vec<u8>, /* Body */
	) {
		match path.first().copied() {
			Some("thumbnail") => {
				if path.len() != 2 {
					return (
						400,
						"text/html",
						b"Bad Request: Invalid number of parameters".to_vec(),
					);
				}

				let filename = Path::new(&self.config.data_directory())
					.join("thumbnails")
					.join(path[1] /* file_cas_id */)
					.with_extension("webp");
				match File::open(&filename) {
					Ok(mut file) => {
						let mut buf = match std::fs::metadata(&filename) {
							Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
							Err(_) => Vec::new(),
						};

						file.read_to_end(&mut buf).unwrap();
						(200, "image/webp", buf)
					}
					Err(_) => (404, "text/html", b"File Not Found".to_vec()),
				}
			}
			_ => (
				400,
				"text/html",
				b"Bad Request: Invalid operation!".to_vec(),
			),
		}
	}

	pub fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		block_on(self.jobs.pause());
		info!("Shutdown complete.");
	}
}
