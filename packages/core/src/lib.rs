use anyhow::Result;
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
		state.read_disk().unwrap_or(error!("No client state found, creating new one..."));

		state.save();

		let core = Core { event_sender, state };
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
				match library::loader::load(&library.library_path, &library.library_id).await {
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
	pub async fn command(&self, cmd: ClientCommand) -> Result<CoreResponse, CoreError> {
		info!("Core command: {:?}", cmd);
		Ok(CoreResponse::Success)
	}
	// query sources of data
	pub async fn query(&self, query: ClientQuery) -> Result<CoreResponse, CoreError> {
		info!("Core query: {:?}", query);
		let response = match query {
			ClientQuery::SysGetVolumes => CoreResponse::SysGetVolumes(sys::volumes::get()?),
			ClientQuery::SysGetLocation { id } => CoreResponse::SysGetLocations(sys::locations::get_location(id).await?),
			ClientQuery::LibGetExplorerDir { path: _, limit: _ } => todo!(),
			ClientQuery::ClientGetState => todo!(),
		};
		Ok(response)
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
	LocScanFull { location_id: i64 },
	FileScanQuick { file_id: i64 },
	FileScanFull { file_id: i64 },
	FileDelete { file_id: i64 },
	TagCreate { name: String, color: String },
	TagAssign { file_id: i64, tag_id: i64 },
	TagDelete { tag_id: i64 },
	LocDelete { location_id: i64 },
	LibDelete { library_id: i64 },
	SysVolumeUnmount { volume_id: i64 },
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
	ClientGetState,
	SysGetVolumes,
	SysGetLocation { id: i64 },
	LibGetExplorerDir { path: String, limit: i64 },
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "payload")]
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
	Success,
	SysGetVolumes(Vec<sys::volumes::Volume>),
	SysGetLocations(sys::locations::LocationResource),
}

#[derive(Error, Debug)]
pub enum CoreError {
	#[error("System error")]
	SysError(#[from] sys::SysError),
}

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
