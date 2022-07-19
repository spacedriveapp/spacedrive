use api::{Ctx, Router};
use job::{JobManager, JobReport};
use library::{LibraryConfigWrapped, LibraryManager};
use log::error;
use node::{NodeConfig, NodeConfigManager};
use serde::{Deserialize, Serialize};
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tag::{Tag, TagWithFiles};

use thiserror::Error;
use tokio::{
	fs,
	sync::{
		mpsc::{self, unbounded_channel, UnboundedReceiver, UnboundedSender},
		oneshot,
	},
};
use ts_rs::TS;

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

// a wrapper around external input with a returning sender channel for core to respond
#[derive(Debug)]
pub struct ReturnableMessage<D, R = Result<CoreResponse, CoreError>> {
	data: D,
	return_sender: oneshot::Sender<R>,
}

// core controller is passed to the client to communicate with the core which runs in a dedicated thread
#[derive(Clone)]
pub struct NodeController {
	query_sender: UnboundedSender<ReturnableMessage<ClientQuery>>,
	command_sender: UnboundedSender<ReturnableMessage<ClientCommand>>,
	config: Arc<NodeConfigManager>,
	library_manager: Arc<LibraryManager>,
	jobs: Arc<JobManager>,
}

impl NodeController {
	pub fn get_request_context(&self, library_id: Option<String>) -> Ctx {
		Ctx {
			library_id,
			library_manager: Arc::clone(&self.library_manager),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
		}
	}

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
}

impl Node {
	// create new instance of node, run startup tasks
	pub async fn new(
		data_dir: impl AsRef<Path>,
	) -> (NodeController, mpsc::Receiver<CoreEvent>, Node, Arc<Router>) {
		fs::create_dir_all(&data_dir).await.unwrap();

		let (event_sender, event_recv) = mpsc::channel(100);
		let config = NodeConfigManager::new(data_dir.as_ref().to_owned())
			.await
			.unwrap();
		let jobs = JobManager::new();
		let node_ctx = NodeContext {
			event_sender: event_sender.clone(),
			config: config.clone(),
			jobs: jobs.clone(),
		};

		let router = api::mount();

		let node = Node {
			config,
			library_manager: LibraryManager::new(data_dir.as_ref().join("libraries"), node_ctx)
				.await
				.unwrap(),
			query_channel: unbounded_channel(),
			command_channel: unbounded_channel(),
			jobs,
			event_sender,
		};

		(
			NodeController {
				config: node.config.clone(),
				library_manager: node.library_manager.clone(),
				jobs: node.jobs.clone(),
				query_sender: node.query_channel.0.clone(),
				command_sender: node.command_channel.0.clone(),
			},
			event_recv,
			node,
			router,
		)
	}

	pub fn get_context(&self) -> NodeContext {
		NodeContext {
			event_sender: self.event_sender.clone(),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
		}
	}

	pub async fn start(mut self) {
		loop {
			// listen on global messaging channels for incoming messages
			tokio::select! {
				Some(msg) = self.query_channel.1.recv() => {
					// let res = self.exec_query(msg.data).await;
					// msg.return_sender.send(res).unwrap_or(());
				}
				Some(msg) = self.command_channel.1.recv() => {
					// let res = self.exec_command(msg.data).await;
					// msg.return_sender.send(res).unwrap_or(());
				}
			}
		}
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
		library_id: String,
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
