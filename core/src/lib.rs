use api::{CoreEvent, Ctx, Router};
use job::JobManager;
use library::LibraryManager;
use node::NodeConfigManager;
use std::{path::Path, sync::Arc};
use thiserror::Error;
use tokio::{
	fs::{self, File},
	io::AsyncReadExt,
	sync::broadcast,
};
use tracing::{error, info};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

pub mod api;
pub(crate) mod job;
pub(crate) mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod util;
pub(crate) mod volume;

pub(crate) mod prisma;

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
	pub async fn new(data_dir: impl AsRef<Path>) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
		let data_dir = data_dir.as_ref();
		fs::create_dir_all(&data_dir).await?;

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
					), // .add_directive(
				    // 	"rspc=debug"
				    // 		.parse()
				    // 		.expect("Error invalid tracing directive!"),
				    // ),
			)
			.with(fmt::layer().with_filter(CONSOLE_LOG_FILTER))
			.init();

		let event_bus = broadcast::channel(1024);
		let config = NodeConfigManager::new(data_dir.to_path_buf()).await?;

		let jobs = JobManager::new();
		let library_manager = LibraryManager::new(
			data_dir.join("libraries"),
			NodeContext {
				config: Arc::clone(&config),
				jobs: Arc::clone(&jobs),
				event_bus_tx: event_bus.0.clone(),
			},
		)
		.await?;

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
			library_manager,
			jobs,
			event_bus,
		};

		Ok((Arc::new(node), router))
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
	pub async fn handle_custom_uri(
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
				match File::open(&filename).await {
					Ok(mut file) => {
						let mut buf = match fs::metadata(&filename).await {
							Ok(metadata) => Vec::with_capacity(metadata.len() as usize),
							Err(_) => Vec::new(),
						};

						file.read_to_end(&mut buf).await.unwrap();
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

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.jobs.pause().await;
		info!("Spacedrive Core shutdown successful!");
	}
}

/// Error type for Node related errors.
#[derive(Error, Debug)]
pub enum NodeError {
	#[error("Failed to create data directory: {0}")]
	FailedToCreateDataDirectory(#[from] std::io::Error),
	#[error("Failed to initialize config: {0}")]
	FailedToInitializeConfig(#[from] node::NodeConfigError),
	#[error("Failed to initialize library manager: {0}")]
	FailedToInitializeLibraryManager(#[from] library::LibraryManagerError),
}
