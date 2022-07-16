use crate::{file::cas::FileIdentifierJob, prisma::file as prisma_file, prisma::location};
use job::{JobManager, JobReport};
use library::{LibraryConfig, LibraryConfigWrapped, LibraryManager};
use node::{NodeConfig, NodeConfigManager};
use serde::{Deserialize, Serialize};
use std::{
	fs,
	path::{Path, PathBuf},
	sync::Arc,
};
use tag::{Tag, TagWithFiles};

use thiserror::Error;
use tokio::sync::{
	mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
	oneshot,
};
use ts_rs::TS;

use crate::encode::ThumbnailJob;

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
		recv.await.unwrap_or(Err(CoreError::QueryError))
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
			println!("Failed to emit event. {:?}", e);
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
}

impl Node {
	// create new instance of node, run startup tasks
	pub async fn new(data_dir: PathBuf) -> (NodeController, mpsc::Receiver<CoreEvent>, Node) {
		fs::create_dir_all(&data_dir).unwrap();

		let (event_sender, event_recv) = mpsc::channel(100);
		let config = NodeConfigManager::new(data_dir.clone()).await.unwrap();
		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			event_sender: event_sender.clone(),
			config: config.clone(),
			jobs: jobs.clone(),
		};

		let node = Node {
			config,
			library_manager: LibraryManager::new(Path::new(&data_dir).join("libraries"), node_ctx)
				.await
				.unwrap(),
			query_channel: unbounded_channel(),
			command_channel: unbounded_channel(),
			jobs,
			event_sender,
		};

		(
			NodeController {
				query_sender: node.query_channel.0.clone(),
				command_sender: node.command_channel.0.clone(),
			},
			event_recv,
			node,
		)
	}

	pub fn get_context(&self) -> NodeContext {
		NodeContext {
			event_sender: self.event_sender.clone(),
			config: self.config.clone(),
			jobs: self.jobs.clone(),
		}
	}

	pub async fn start(mut self) {
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
			}
		}
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
					LibraryCommand::SysVolumeUnmount { id: _ } => todo!(),
					LibraryCommand::GenerateThumbsForLocation { id, path } => {
						ctx.spawn_job(Box::new(ThumbnailJob {
							location_id: id,
							path,
							background: false, // fix
						}))
						.await;
						CoreResponse::Success(())
					}
					LibraryCommand::IdentifyUniqueFiles { id, path } => {
						ctx.spawn_job(Box::new(FileIdentifierJob {
							location_id: id,
							path,
						}))
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
				let ctx = match self.library_manager.get_ctx(library_id.clone()).await {
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
					LibraryQuery::LibGetExplorerDir {
						path,
						location_id,
						limit: _,
					} => CoreResponse::LibGetExplorerDir(
						file::explorer::open_dir(&ctx, &location_id, &path).await?,
					),
					LibraryQuery::JobGetHistory => {
						CoreResponse::JobGetHistory(JobManager::get_history(&ctx).await?)
					}
					LibraryQuery::GetLibraryStatistics => CoreResponse::GetLibraryStatistics(
						library::Statistics::calculate(&ctx).await?,
					),
					LibraryQuery::LibGetTags { name_starts_with } => {
						tag::get_all_tags(ctx, name_starts_with).await?
					}
					LibraryQuery::LibGetFilesTagged { tag_id } => {
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
		id: String,
		name: Option<String>,
		description: Option<String>,
	},
	DeleteLibrary {
		id: String,
	},
	LibraryCommand {
		library_id: String,
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
		path: String,
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
	SysVolumeUnmount {
		id: i32,
	},
	GenerateThumbsForLocation {
		id: i32,
		path: String,
	},
	// PurgeDatabase,
	IdentifyUniqueFiles {
		id: i32,
		path: String,
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
		library_id: String,
		query: LibraryQuery,
	},
}

/// is a query destined for a specific library which is loaded into the core.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum LibraryQuery {
	JobGetHistory,
	GetLocations,
	GetLocation {
		id: i32,
	},
	GetRunningJobs,
	LibGetExplorerDir {
		location_id: i32,
		path: String,
		limit: i32,
	},
	GetLibraryStatistics,
	LibGetTags {
		name_starts_with: Option<String>,
	},
	LibGetFilesTagged {
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
	TagCreateResponse(tag::Tag),
	GetTag(Option<tag::Tag>),
	GetTags(Vec<tag::Tag>),
	GetLocation(sys::LocationResource),
	GetLocations(Vec<sys::LocationResource>),
	LibGetExplorerDir(file::DirectoryWithContents),
	GetNode(NodeState),
	LocCreate(sys::LocationResource),
	OpenTag(Vec<tag::TagWithFiles>),
	GetRunningJobs(Vec<JobReport>),
	JobGetHistory(Vec<JobReport>),
	GetLibraryStatistics(library::Statistics),
}

#[derive(Error, Debug)]
pub enum CoreError {
	#[error("Query error")]
	QueryError,
	#[error("System error")]
	SysError(#[from] sys::SysError),
	#[error("File error")]
	FileError(#[from] file::FileError),
	#[error("Job error")]
	JobError(#[from] job::JobError),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("Database error")]
	LibraryError(#[from] library::LibraryError),
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export)]
pub enum CoreResource {
	Client,
	Library,
	Location(sys::LocationResource),
	File(file::File),
	Job(JobReport),
	Tag,
}
