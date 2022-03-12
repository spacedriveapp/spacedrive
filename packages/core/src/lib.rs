use anyhow::Result;
use log::{error, info};
use once_cell::sync::OnceCell;
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
    pub event_channel_sender: mpsc::Sender<ClientEvent>,
}

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("System error")]
    SysError(#[from] sys::SysError),
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(rename_all = "snake_case", tag = "key", content = "payload")]
#[ts(export)]
pub enum ClientEvent {
    NewFileTypeThumb { file_id: u32, icon_created: bool },
    NewJobCreated { job_id: u32, progress: u8 },
    ResourceChange { key: String, id: String },
    DatabaseDisconnected { reason: Option<String> },
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
    SysGetVolumes,
    ClientGetCurrent,
    SysGetLocations { id: String },
    LibGetExplorerDir { path: String, limit: u32 },
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "key", content = "data")]
#[ts(export)]
pub enum ClientResponse {
    SysGetVolumes(Vec<sys::volumes::Volume>),
}

// static configuration passed in by host application
#[derive(Serialize, Deserialize, Debug)]
pub struct CoreConfig {
    pub data_dir: std::path::PathBuf,
    pub primary_db: std::path::PathBuf,
    pub file_type_thumb_dir: std::path::PathBuf,
}

pub static CORE: OnceCell<Core> = OnceCell::new();

impl Core {
    pub async fn query(query: ClientQuery) -> Result<ClientResponse, CoreError> {
        println!("query: {:?}", query);
        let response = match query {
            ClientQuery::SysGetVolumes => ClientResponse::SysGetVolumes(sys::volumes::get()?),
            ClientQuery::SysGetLocations { id: _ } => todo!(),
            ClientQuery::LibGetExplorerDir { path: _, limit: _ } => todo!(),
            ClientQuery::ClientGetCurrent => todo!(),
        };
        Ok(response)
    }
    pub async fn configure(mut data_dir: std::path::PathBuf) -> mpsc::Receiver<ClientEvent> {
        data_dir = data_dir.join("spacedrive");

        let (event_sender, event_receiver) = mpsc::channel(100);
        let _ = CORE.set(Core {
            event_channel_sender: event_sender,
        });

        let data_dir = data_dir.to_str().unwrap();
        // create data directory if it doesn't exist
        fs::create_dir_all(&data_dir).unwrap();
        // prepare basic client state
        let mut client_config =
            ClientState::new(data_dir, "diamond-mastering-space-dragon").unwrap();
        // load from disk
        client_config
            .read_disk()
            .unwrap_or(error!("No client state found, creating new one..."));

        client_config.save();

        // begin asynchronous startup routines
        info!("Starting up... {:?}", client_config);
        if client_config.libraries.len() == 0 {
            match library::loader::create(None).await {
                Ok(library) => info!("Created new library: {:?}", library),
                Err(e) => info!("Error creating library: {:?}", e),
            }
        } else {
            for library in client_config.libraries.iter() {
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
        // activate p2p listeners
        // p2p::listener::listen(None);
        event_receiver
    }
}
