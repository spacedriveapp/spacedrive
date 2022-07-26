use crate::{
	encode::{ThumbnailJob, ThumbnailJobInit},
	file::cas::{FileIdentifierJob, FileIdentifierJobInit},
	job::{Job, JobManager, JobReport},
	library::{LibraryConfig, LibraryConfigWrapped, LibraryManager},
	node::{NodeConfig, NodeConfigManager},
	prisma::file as prisma_file,
	prisma::location,
	tag::{Tag, TagWithFiles},
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use thiserror::Error;
use tokio::{
	fs,
	sync::{
		mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
		oneshot,
	},
};
use ts_rs::TS;
use uuid::Uuid;

mod encode;
mod file;
mod job;
mod library;
mod node;
mod prisma;
mod sys;
mod tag;
mod util;

// a wrapper around external input with a returning sender channel for core to respond
#[derive(Debug)]
pub struct ReturnableMessage<D, R = Result<CoreResponse, CoreError>> {
	data: D,
	return_sender: oneshot::Sender<R>,
}

// core controller is passed to the client to communicate with the core which runs in a dedicated thread
pub struct NodeController {
	query_sender: UnboundedSender<ReturnableMessage<ClientQuery>>,
	command_sender: UnboundedSender<ReturnableMessage<ClientCommand>>,
}

impl NodeController {
	pub async fn query(&self, query: ClientQuery) -> Result<CoreResponse, CoreError> {
		// a one time use channel to send and await a response
		let (sender, recv) = oneshot::channel();
		self.query_sender
			.send(ReturnableMessage {
				data: query,
				return_sender: sender,
			})
			.unwrap_or(());
		// wait for response and return
		recv.await.unwrap_or(Err(CoreError::Query))
	}

	pub async fn command(&self, command: ClientCommand) -> Result<CoreResponse, CoreError> {
		let (sender, recv) = oneshot::channel();
		self.command_sender
			.send(ReturnableMessage {
				data: command,
				return_sender: sender,
			})
			.unwrap_or(());

		recv.await.unwrap()
	}
}

#[derive(Clone)]
pub struct NodeContext {
	pub event_sender: mpsc::Sender<CoreEvent>,
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
}

impl NodeContext {
	pub async fn emit(&self, event: CoreEvent) {
		self.event_sender.send(event).await.unwrap_or_else(|e| {
			error!("Failed to emit event. {:#?}", e);
		});
	}
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	library_manager: Arc<LibraryManager>,
	jobs: Arc<JobManager>,

	// global messaging channels
	query_channel: (
		UnboundedSender<ReturnableMessage<ClientQuery>>,
		UnboundedReceiver<ReturnableMessage<ClientQuery>>,
	),
	command_channel: (
		UnboundedSender<ReturnableMessage<ClientCommand>>,
		UnboundedReceiver<ReturnableMessage<ClientCommand>>,
	),
	event_sender: mpsc::Sender<CoreEvent>,
	shutdown_completion_tx: oneshot::Sender<()>,
}

impl Node {
	// create new instance of node, run startup tasks
	pub async fn new(
		data_dir: impl AsRef<Path>,
	) -> (
		NodeController,
		mpsc::Receiver<CoreEvent>,
		Node,
		oneshot::Receiver<()>,
	) {
		let data_dir = data_dir.as_ref();
		fs::create_dir_all(data_dir).await.unwrap();

		let (event_sender, event_recv) = mpsc::channel(100);
		let config = NodeConfigManager::new(data_dir.to_owned()).await.unwrap();

		let (shutdown_completion_tx, shutdown_completion_rx) = oneshot::channel();

		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			event_sender: event_sender.clone(),
			config: config.clone(),
			jobs: jobs.clone(),
		};
		let library_manager = LibraryManager::new(data_dir.join("libraries"), node_ctx)
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

		let node = Node {
			config,
			library_manager,
			query_channel: unbounded_channel(),
			command_channel: unbounded_channel(),
			jobs,
			event_sender,
			shutdown_completion_tx,
		};

		(
			NodeController {
				query_sender: node.query_channel.0.clone(),
				command_sender: node.command_channel.0.clone(),
			},
			event_recv,
			node,
			shutdown_completion_rx,
		)
	}

	pub fn get_context(&self) -> NodeContext {
		NodeContext {
			event_sender: self.event_sender.clone(),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
		}
	}

	pub async fn start(mut self, mut shutdown_rx: oneshot::Receiver<()>) {
		loop {
			// listen on global messaging channels for incoming messages
			tokio::select! {
				Some(msg) = self.query_channel.1.recv() => {
					let res = self.exec_query(msg.data).await;
					msg.return_sender.send(res).unwrap_or(());
				}
				Some(msg) = self.command_channel.1.recv() => {
					let res = self.exec_command(msg.data).await;
					msg.return_sender.send(res).unwrap_or(());
				}

				_ = &mut shutdown_rx => {
					info!("Initiating shutdown node...");
					self.shutdown().await;
					info!("Node shutdown complete.");
					self.shutdown_completion_tx.send(())
						.expect("critical error: failed to send node shutdown completion signal");

					break;
				}
			}
		}
	}

	pub async fn shutdown(&self) {
		self.jobs.pause().await
	}

	async fn exec_command(&mut self, cmd: ClientCommand) -> Result<CoreResponse, CoreError> {
		Ok(match cmd {
			ClientCommand::CreateLibrary { name } => {
				self.library_manager
					.create(LibraryConfig {
						name: name.to_string(),
						..Default::default()
					})
					.await
					.unwrap();
				CoreResponse::Success(())
			}
			ClientCommand::EditLibrary {
				id,
				name,
				description,
			} => {
				self.library_manager
					.edit(id, name, description)
					.await
					.unwrap();
				CoreResponse::Success(())
			}
			ClientCommand::DeleteLibrary { id } => {
				self.library_manager.delete_library(id).await.unwrap();
				CoreResponse::Success(())
			}
			ClientCommand::LibraryCommand {
				library_id,
				command,
			} => {
				let ctx = self.library_manager.get_ctx(library_id).await.unwrap();
				match command {
					// CRUD for locations
					LibraryCommand::LocCreate { path } => {
						let loc = sys::new_location_and_scan(&ctx, &path).await?;
						// ctx.queue_job(Box::new(FileIdentifierJob));
						CoreResponse::LocCreate(loc)
					}
					LibraryCommand::LocUpdate { id, name } => {
						ctx.db
							.location()
							.find_unique(location::id::equals(id))
							.update(vec![location::name::set(name)])
							.exec()
							.await?;

						CoreResponse::Success(())
					}
					LibraryCommand::LocDelete { id } => {
						sys::delete_location(&ctx, id).await?;
						CoreResponse::Success(())
					}
					LibraryCommand::LocFullRescan { id } => {
						sys::scan_location(&ctx, id, String::new()).await;
						CoreResponse::Success(())
					}
					LibraryCommand::LocQuickRescan { id: _ } => todo!(),
					// CRUD for files
					LibraryCommand::FileReadMetaData { id: _ } => todo!(),
					LibraryCommand::FileSetNote { id, note } => {
						file::set_note(ctx, id, note).await?
					}
					LibraryCommand::FileSetFavorite { id, favorite } => {
						file::favorite(ctx, id, favorite).await?
					}
					// ClientCommand::FileEncrypt { id: _, algorithm: _ } => todo!(),
					LibraryCommand::FileDelete { id } => {
						ctx.db
							.file()
							.find_unique(prisma_file::id::equals(id))
							.delete()
							.exec()
							.await?;

						CoreResponse::Success(())
					}
					// CRUD for tags
					LibraryCommand::TagCreate { name, color } => {
						tag::create_tag(ctx, name, color).await?
					}
					LibraryCommand::TagAssign { file_id, tag_id } => {
						tag::tag_assign(ctx, file_id, tag_id).await?
					}
					LibraryCommand::TagDelete { id } => tag::tag_delete(ctx, id).await?,
					LibraryCommand::TagUpdate { id, name, color } => {
						tag::update_tag(ctx, id, name, color).await?
					}
					// CRUD for libraries
					LibraryCommand::VolUnmount { id: _ } => todo!(),
					LibraryCommand::GenerateThumbsForLocation { id, path } => {
						ctx.spawn_job(Job::new(
							ThumbnailJobInit {
								location_id: id,
								path,
								background: false, // fix
							},
							Box::new(ThumbnailJob {}),
						))
						.await;
						CoreResponse::Success(())
					}
					LibraryCommand::IdentifyUniqueFiles { id, path } => {
						ctx.spawn_job(Job::new(
							FileIdentifierJobInit {
								location_id: id,
								path,
							},
							Box::new(FileIdentifierJob {}),
						))
						.await;
						CoreResponse::Success(())
					}
				}
			}
		})
	}

	// query sources of data
	async fn exec_query(&self, query: ClientQuery) -> Result<CoreResponse, CoreError> {
		Ok(match query {
			ClientQuery::GetLibraries => {
				CoreResponse::GetLibraries(self.library_manager.get_all_libraries_config().await)
			}
			ClientQuery::GetNode => CoreResponse::GetNode(NodeState {
				config: self.config.get().await,
				data_path: self.config.data_directory().to_str().unwrap().to_string(),
			}),
			ClientQuery::GetNodes => todo!(),
			ClientQuery::GetVolumes => CoreResponse::GetVolumes(sys::Volume::get_volumes()?),
			ClientQuery::LibraryQuery { library_id, query } => {
				let ctx = match self.library_manager.get_ctx(library_id).await {
					Some(ctx) => ctx,
					None => {
						println!("Library '{}' not found!", library_id);
						return Ok(CoreResponse::Error("Library not found".into()));
					}
				};
				match query {
					LibraryQuery::GetLocations => {
						CoreResponse::GetLocations(sys::get_locations(&ctx).await?)
					}
					LibraryQuery::GetRunningJobs => {
						CoreResponse::GetRunningJobs(self.jobs.get_running().await)
					}
					// get location from library
					LibraryQuery::GetLocation { id } => {
						CoreResponse::GetLocation(sys::get_location(&ctx, id).await?)
					}
					// return contents of a directory for the explorer
					LibraryQuery::GetExplorerDir {
						location_id,
						path,
						limit: _,
					} => CoreResponse::GetExplorerDir(Box::new(
						file::explorer::open_dir(&ctx, location_id, path).await?,
					)),
					LibraryQuery::GetJobHistory => {
						CoreResponse::GetJobHistory(JobManager::get_history(&ctx).await?)
					}
					LibraryQuery::GetLibraryStatistics => CoreResponse::GetLibraryStatistics(
						library::Statistics::calculate(&ctx).await?,
					),
					LibraryQuery::GetTags => tag::get_all_tags(ctx).await?,
					LibraryQuery::GetFilesTagged { tag_id } => {
						tag::get_files_for_tag(ctx, tag_id).await?
					}
				}
			}
		})
	}
}

/// is a command destined for the core
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientCommand {
	// Libraries
	CreateLibrary {
		name: String,
	},
	EditLibrary {
		id: Uuid,
		name: Option<String>,
		description: Option<String>,
	},
	DeleteLibrary {
		id: Uuid,
	},
	LibraryCommand {
		library_id: Uuid,
		command: LibraryCommand,
	},
}

/// is a command destined for a specific library which is loaded into the core.
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum LibraryCommand {
	// Files
	FileReadMetaData {
		id: i32,
	},
	FileSetNote {
		id: i32,
		note: Option<String>,
	},
	FileSetFavorite {
		id: i32,
		favorite: bool,
	},
	// FileEncrypt { id: i32, algorithm: EncryptionAlgorithm },
	FileDelete {
		id: i32,
	},
	// Tags
	TagCreate {
		name: String,
		color: String,
	},
	TagUpdate {
		id: i32,
		name: Option<String>,
		color: Option<String>,
	},
	TagAssign {
		file_id: i32,
		tag_id: i32,
	},
	TagDelete {
		id: i32,
	},
	// Locations
	LocCreate {
		path: PathBuf,
	},
	LocUpdate {
		id: i32,
		name: Option<String>,
	},
	LocDelete {
		id: i32,
	},
	LocFullRescan {
		id: i32,
	},
	LocQuickRescan {
		id: i32,
	},
	// System
	VolUnmount {
		id: i32,
	},
	GenerateThumbsForLocation {
		id: i32,
		path: PathBuf,
	},
	// PurgeDatabase,
	IdentifyUniqueFiles {
		id: i32,
		path: PathBuf,
	},
}

/// is a query destined for the core
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
	GetLibraries,
	GetNode,
	GetVolumes,
	GetNodes,
	LibraryQuery {
		library_id: Uuid,
		query: LibraryQuery,
	},
}

/// is a query destined for a specific library which is loaded into the core.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum LibraryQuery {
	GetJobHistory,
	GetLocations,
	GetLocation {
		id: i32,
	},
	GetRunningJobs,
	GetExplorerDir {
		location_id: i32,
		path: PathBuf,
		limit: i32,
	},
	GetLibraryStatistics,
	GetTags,
	GetFilesTagged {
		tag_id: i32,
	},
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum CoreEvent {
	// most all events should be once of these two
	InvalidateQuery(ClientQuery),
	InvalidateQueryDebounced(ClientQuery),
	InvalidateResource(CoreResource),
	NewThumbnail { cas_id: String },
	Log { message: String },
	DatabaseDisconnected { reason: Option<String> },
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct NodeState {
	#[serde(flatten)]
	pub config: NodeConfig,
	pub data_path: String,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum CoreResponse {
	Success(()),
	Error(String),
	GetLibraries(Vec<LibraryConfigWrapped>),
	GetVolumes(Vec<sys::Volume>),
	TagCreateResponse(Tag),
	GetTag(Option<Tag>),
	GetTags(Vec<Tag>),
	GetLocation(sys::LocationResource),
	GetLocations(Vec<sys::LocationResource>),
	GetExplorerDir(Box<file::DirectoryWithContents>),
	GetNode(NodeState),
	LocCreate(sys::LocationResource),
	OpenTag(Vec<TagWithFiles>),
	GetRunningJobs(Vec<JobReport>),
	GetJobHistory(Vec<JobReport>),
	GetLibraryStatistics(library::Statistics),
}

#[derive(Error, Debug)]
pub enum CoreError {
	#[error("Query error")]
	Query,
	#[error("System error: {0}")]
	Sys(#[from] sys::SysError),
	#[error("File error: {0}")]
	File(#[from] file::FileError),
	#[error("Job error: {0}")]
	Job(#[from] job::JobError),
	#[error("Database error: {0}")]
	Database(#[from] prisma::QueryError),
	#[error("Library error: {0}")]
	Library(#[from] library::LibraryError),
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum CoreResource {
	Client,
	Library,
	Location(sys::LocationResource),
	File(file::File),
	Job(JobReport),
	Tag(Tag),
}
