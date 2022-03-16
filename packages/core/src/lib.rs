use anyhow::Result;
use crypto::encryption::EncryptionAlgorithm;
use job::JobResource;
use log::{error, info};
use serde::{Deserialize, Serialize};
use state::client::ClientState;
use std::fs;
use thiserror::Error;
use tokio::sync::mpsc;
use ts_rs::TS;

// init modules
pub mod client;
pub mod crypto;
pub mod db;
pub mod file;
pub mod job;
pub mod library;
pub mod p2p;
pub mod prisma;
pub mod state;
pub mod sys;
pub mod util;
// pub mod native;

pub struct Core {
	pub event_sender: mpsc::Sender<CoreEvent>,
	pub state: ClientState,
	pub job_runner: job::JobRunner,
}

impl Core {
	// create new instance of core, run startup tasks
	pub async fn new(mut data_dir: std::path::PathBuf) -> (Core, mpsc::Receiver<CoreEvent>) {
		let (event_sender, event_receiver) = mpsc::channel(100);

		data_dir = data_dir.join("spacedrive");
		let data_dir = data_dir.to_str().unwrap();
		// create data directory if it doesn't exist
		fs::create_dir_all(&data_dir).unwrap();
		// prepare basic client state
		let mut state = ClientState::new(data_dir, "diamond-mastering-space-dragon").unwrap();
		// load from disk
		state
			.read_disk()
			.unwrap_or(error!("No client state found, creating new one..."));

		state.save();

		let core = Core {
			event_sender,
			state,
			job_runner: job::JobRunner::new(),
		};

		core.initializer().await;

		(core, event_receiver)
		// activate p2p listeners
		// p2p::listener::listen(None);
	}
	// load library database + initialize client with db
	pub async fn initializer(&self) {
		if self.state.libraries.len() == 0 {
			match library::loader::create(None).await {
				Ok(library) => info!("Created new library: {:?}", library),
				Err(e) => info!("Error creating library: {:?}", e),
			}
		} else {
			for library in self.state.libraries.iter() {
				// init database for library
				match library::loader::load(&library.library_path, &library.library_uuid).await {
					Ok(library) => info!("Loaded library: {:?}", library),
					Err(e) => info!("Error loading library: {:?}", e),
				}
			}
		}
		// init client
		match client::create().await {
			Ok(_) => info!("Spacedrive online"),
			Err(e) => info!("Error initializing client: {:?}", e),
		};
	}
	pub fn queue(&mut self, job: JobResource) -> &mut JobResource {
		self.job_runner.queued_jobs.push(job);
		self.job_runner.queued_jobs.last_mut().unwrap()
	}
	pub async fn command(&self, cmd: ClientCommand) -> Result<CoreResponse, CoreError> {
		info!("Core command: {:?}", cmd);
		Ok(match cmd {
			// CRUD for locations
			ClientCommand::LocCreate { id } => todo!(),
			ClientCommand::LocUpdate { id: _, name: _ } => todo!(),
			ClientCommand::LocDelete { id: _ } => todo!(),
			// CRUD for files
			ClientCommand::FileRead { id: _ } => todo!(),
			ClientCommand::FileEncrypt { id: _, algorithm: _ } => todo!(),
			ClientCommand::FileDelete { id: _ } => todo!(),
			// CRUD for tags
			ClientCommand::TagCreate { name: _, color: _ } => todo!(),
			ClientCommand::TagAssign { file_id: _, tag_id: _ } => todo!(),
			ClientCommand::TagDelete { id: _ } => todo!(),
			// CRUD for libraries
			ClientCommand::SysVolumeUnmount { id: _ } => todo!(),
			ClientCommand::LibDelete { id: _ } => todo!(),
			ClientCommand::TagUpdate { name, color } => todo!(),
		})
	}
	// query sources of data
	pub async fn query(&self, query: ClientQuery) -> Result<CoreResponse, CoreError> {
		info!("Core query: {:?}", query);
		Ok(match query {
			// return the client state from memory
			ClientQuery::ClientGetState => CoreResponse::ClientGetState(self.state.clone()),
			// get system volumes without saving to library
			ClientQuery::SysGetVolumes => CoreResponse::SysGetVolumes(sys::volumes::get_volumes()?),
			// get location from library
			ClientQuery::SysGetLocation { id } => CoreResponse::SysGetLocation(sys::locations::get_location(id).await?),
			// return contents of a directory for the explorer
			ClientQuery::LibGetExplorerDir { path, limit: _ } => {
				CoreResponse::LibGetExplorerDir(file::explorer::open_dir(&path).await?)
			},
			ClientQuery::LibGetTags => todo!(),
			ClientQuery::JobGetRunning => CoreResponse::JobGetRunning(job::JobResource::get_running().await?),
			ClientQuery::JobGetHistory => CoreResponse::JobGetHistory(job::JobResource::get_history().await?),
		})
	}
	// send an event to the client

	pub async fn send(&self, event: CoreEvent) {
		self.event_sender.send(event).await.unwrap();
	}
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientCommand {
	// Files
	FileRead { id: i64 },
	FileEncrypt { id: i64, algorithm: EncryptionAlgorithm },
	FileDelete { id: i64 },
	// Library
	LibDelete { id: i64 },
	// Tags
	TagCreate { name: String, color: String },
	TagUpdate { name: String, color: String },
	TagAssign { file_id: i64, tag_id: i64 },
	TagDelete { id: i64 },
	// Locations
	LocCreate { id: i64 },
	LocUpdate { id: i64, name: Option<String> },
	LocDelete { id: i64 },
	// System
	SysVolumeUnmount { id: i64 },
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
	ClientGetState,
	SysGetVolumes,
	LibGetTags,
	JobGetRunning,
	JobGetHistory,
	SysGetLocation { id: i64 },
	LibGetExplorerDir { path: String, limit: i64 },
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum CoreEvent {
	// most all events should be once of these two
	InvalidateQuery(ClientQuery),
	InvalidateResource(CoreResource),
	Log { message: String },
	DatabaseDisconnected { reason: Option<String> },
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum CoreResponse {
	Success(()),
	SysGetVolumes(Vec<sys::volumes::Volume>),
	SysGetLocation(sys::locations::LocationResource),
	LibGetExplorerDir(file::explorer::Directory),
	ClientGetState(ClientState),
	LocCreate(sys::locations::LocationResource),
	JobGetRunning(Vec<job::JobResource>),
	JobGetHistory(Vec<job::JobResource>),
}

#[derive(Error, Debug)]
pub enum CoreError {
	#[error("System error")]
	SysError(#[from] sys::SysError),
	#[error("File error")]
	FileError(#[from] file::FileError),
	#[error("Job error")]
	JobError(#[from] job::JobError),
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}

// this does nothing yet, but maybe we could use these to invalidate queries tied to resources
#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub enum CoreResource {
	Client,
	Library,
	Location,
	File,
	Job,
	Tag,
}
