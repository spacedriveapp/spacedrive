use crate::{file::cas::FileIdentifierJob, prisma::file as prisma_file, prisma::location};
use job::{Job, JobReport, Jobs};
use library::{LibraryConfig, LibraryConfigWrapped, LibraryManager};
use node::{NodeConfig, NodeConfigManager};
use serde::{Deserialize, Serialize};
use std::{
	fs,
	path::{Path, PathBuf},
	sync::Arc,
};
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

#[derive(Debug)]
pub enum InternalEvent {
	JobIngest(Box<dyn Job>),
	JobQueue(Box<dyn Job>),
	JobComplete(String),
}

#[derive(Clone)]
pub struct NodeContext {
	pub event_sender: mpsc::Sender<CoreEvent>,
	pub internal_sender: UnboundedSender<InternalEvent>,
	pub config: Arc<NodeConfigManager>,
}

impl NodeContext {
	pub fn spawn_job(&self, job: Box<dyn Job>) {
		self.internal_sender
			.send(InternalEvent::JobIngest(job))
			.unwrap_or_else(|e| {
				println!("Failed to spawn job. {:?}", e);
			});
	}

	pub fn queue_job(&self, job: Box<dyn Job>) {
		self.internal_sender
			.send(InternalEvent::JobQueue(job))
			.unwrap_or_else(|e| {
				println!("Failed to queue job. {:?}", e);
			});
	}

	pub async fn emit(&self, event: CoreEvent) {
		self.event_sender.send(event).await.unwrap_or_else(|e| {
			println!("Failed to emit event. {:?}", e);
		});
	}
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	library_manager: Arc<LibraryManager>,
	jobs: job::Jobs,

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

	// a channel for child threads to send events back to the core
	internal_channel: (
		UnboundedSender<InternalEvent>,
		UnboundedReceiver<InternalEvent>,
	),
}

impl Node {
	// create new instance of node, run startup tasks
	pub async fn new(data_dir: PathBuf) -> (NodeController, mpsc::Receiver<CoreEvent>, Node) {
		fs::create_dir_all(&data_dir).unwrap();

		let (event_sender, event_recv) = mpsc::channel(100);

		let internal_channel = unbounded_channel();
		let config = NodeConfigManager::new(data_dir.clone()).await.unwrap();

		let node_ctx = NodeContext {
			event_sender: event_sender.clone(),
			internal_sender: internal_channel.0.clone(),
			config: config.clone(),
		};

		let node = Node {
			config,
			library_manager: LibraryManager::new(Path::new(&data_dir).join("libraries"), node_ctx)
				.await
				.unwrap(),
			query_channel: unbounded_channel(),
			command_channel: unbounded_channel(),
			jobs: Jobs::new(),
			event_sender,
			internal_channel,
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
			internal_sender: self.internal_channel.0.clone(),
			config: self.config.clone(),
		}
	}

	pub async fn start(mut self) {
		let ctx = self.library_manager.get_ctx().await.unwrap();
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
				Some(event) = self.internal_channel.1.recv() => {
					match event {
						InternalEvent::JobIngest(job) => {
							self.jobs.ingest(&ctx, job).await;
						},
						InternalEvent::JobQueue(job) => {
							self.jobs.ingest_queue(&ctx, job);
						},
						InternalEvent::JobComplete(id) => {
							self.jobs.complete(&ctx, id).await;
						},
					}
				}
			}
		}
	}

	async fn exec_command(&mut self, cmd: ClientCommand) -> Result<CoreResponse, CoreError> {
		let ctx = self.library_manager.get_ctx().await.unwrap();
		Ok(match cmd {
			ClientCommand::CreateLibrary { name } => {
				self.library_manager
					.create(LibraryConfig {
						name: name.to_string(),
						..Default::default()
					})
					.await
					.unwrap();

				ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::NodeGetLibraries))
					.await;

				CoreResponse::Success(())
			}
			ClientCommand::EditLibrary { name, description } => {
				self.library_manager
					.edit_library(&ctx, name, description)
					.await
					.unwrap();
				CoreResponse::Success(())
			}
			ClientCommand::DeleteLibrary { id } => {
				self.library_manager.delete_library(&ctx, id).await.unwrap();
				CoreResponse::Success(())
			}
			// CRUD for locations
			ClientCommand::LocCreate { path } => {
				let loc = sys::new_location_and_scan(&ctx, &path).await?;
				// ctx.queue_job(Box::new(FileIdentifierJob));
				CoreResponse::LocCreate(loc)
			}
			ClientCommand::LocUpdate { id, name } => {
				ctx.db
					.location()
					.find_unique(location::id::equals(id))
					.update(vec![location::name::set(name)])
					.exec()
					.await?;

				CoreResponse::Success(())
			}
			ClientCommand::LocDelete { id } => {
				sys::delete_location(&ctx, id).await?;
				CoreResponse::Success(())
			}
			ClientCommand::LocRescan { id } => {
				sys::scan_location(&ctx, id, String::new());

				CoreResponse::Success(())
			}
			// CRUD for files
			ClientCommand::FileReadMetaData { id: _ } => todo!(),
			ClientCommand::FileSetNote { id, note } => file::set_note(ctx, id, note).await?,
			// ClientCommand::FileEncrypt { id: _, algorithm: _ } => todo!(),
			ClientCommand::FileDelete { id } => {
				ctx.db
					.file()
					.find_unique(prisma_file::id::equals(id))
					.delete()
					.exec()
					.await?;

				CoreResponse::Success(())
			}
			// CRUD for tags
			ClientCommand::TagCreate { name: _, color: _ } => todo!(),
			ClientCommand::TagAssign {
				file_id: _,
				tag_id: _,
			} => todo!(),
			ClientCommand::TagDelete { id: _ } => todo!(),
			// CRUD for libraries
			ClientCommand::SysVolumeUnmount { id: _ } => todo!(),
			ClientCommand::LibDelete { id: _ } => todo!(),
			ClientCommand::TagUpdate { name: _, color: _ } => todo!(),
			ClientCommand::GenerateThumbsForLocation { id, path } => {
				ctx.spawn_job(Box::new(ThumbnailJob {
					location_id: id,
					path,
					background: false, // fix
				}));
				CoreResponse::Success(())
			}
			// ClientCommand::PurgeDatabase => {
			//   println!("Purging database...");
			//   fs::remove_file(Path::new(&self.state.data_path).join("library.db")).unwrap();
			//   CoreResponse::Success(())
			// }
			ClientCommand::IdentifyUniqueFiles { id, path } => {
				ctx.spawn_job(Box::new(FileIdentifierJob {
					location_id: id,
					path,
				}));
				CoreResponse::Success(())
			}
		})
	}

	// query sources of data
	async fn exec_query(&self, query: ClientQuery) -> Result<CoreResponse, CoreError> {
		let ctx = self.library_manager.get_ctx().await.unwrap();
		Ok(match query {
			ClientQuery::NodeGetLibraries => CoreResponse::NodeGetLibraries(
				self.library_manager.get_all_libraries_config().await,
			),
			ClientQuery::NodeGetState => CoreResponse::NodeGetState(NodeState {
				config: self.config.get().await,
				data_path: self.config.data_directory().to_str().unwrap().to_string(),
			}),
			// get system volumes without saving to library
			ClientQuery::SysGetVolumes => CoreResponse::SysGetVolumes(sys::Volume::get_volumes()?),
			ClientQuery::SysGetLocations => {
				CoreResponse::SysGetLocations(sys::get_locations(&ctx).await?)
			}
			// get location from library
			ClientQuery::SysGetLocation { id } => {
				CoreResponse::SysGetLocation(sys::get_location(&ctx, id).await?)
			}
			// return contents of a directory for the explorer
			ClientQuery::LibGetExplorerDir {
				path,
				location_id,
				limit: _,
			} => CoreResponse::LibGetExplorerDir(
				file::explorer::open_dir(&ctx, &location_id, &path).await?,
			),
			ClientQuery::LibGetTags => todo!(),
			ClientQuery::JobGetRunning => {
				CoreResponse::JobGetRunning(self.jobs.get_running().await)
			}
			ClientQuery::JobGetHistory => {
				CoreResponse::JobGetHistory(Jobs::get_history(&ctx).await?)
			}
			ClientQuery::GetLibraryStatistics => {
				CoreResponse::GetLibraryStatistics(library::Statistics::calculate(&ctx).await?)
			}
			ClientQuery::GetNodes => todo!(),
		})
	}
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientCommand {
	// Libraries
	CreateLibrary {
		name: String,
	},
	EditLibrary {
		name: Option<String>,
		description: Option<String>,
	},
	DeleteLibrary {
		id: String,
	},
	// Files
	FileReadMetaData {
		id: i32,
	},
	FileSetNote {
		id: i32,
		note: Option<String>,
	},
	// FileEncrypt { id: i32, algorithm: EncryptionAlgorithm },
	FileDelete {
		id: i32,
	},
	// Library
	LibDelete {
		id: i32,
	},
	// Tags
	TagCreate {
		name: String,
		color: String,
	},
	TagUpdate {
		name: String,
		color: String,
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
	LocRescan {
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

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
	NodeGetLibraries,
	NodeGetState,
	SysGetVolumes,
	LibGetTags,
	JobGetRunning,
	JobGetHistory,
	SysGetLocations,
	SysGetLocation {
		id: i32,
	},
	LibGetExplorerDir {
		location_id: i32,
		path: String,
		limit: i32,
	},
	GetLibraryStatistics,
	GetNodes,
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
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
	NodeGetLibraries(Vec<LibraryConfigWrapped>),
	SysGetVolumes(Vec<sys::Volume>),
	SysGetLocation(sys::LocationResource),
	SysGetLocations(Vec<sys::LocationResource>),
	LibGetExplorerDir(file::DirectoryWithContents),
	NodeGetState(NodeState),
	LocCreate(sys::LocationResource),
	JobGetRunning(Vec<JobReport>),
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

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub enum CoreResource {
	Client,
	Library,
	Location(sys::LocationResource),
	File(file::File),
	Job(JobReport),
	Tag,
}
