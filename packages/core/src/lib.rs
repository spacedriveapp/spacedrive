pub mod crypto;
pub mod db;
pub mod file;
pub mod library;
// pub mod native;
pub mod client;
pub mod p2p;
pub mod prisma;
pub mod state;
pub mod sys;
pub mod util;
use anyhow::Result;
use futures::{stream::StreamExt, Stream};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use state::client::ClientState;
use std::fs;
use thiserror::Error;
use tokio::sync::mpsc;
use ts_rs::TS;

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
#[serde(rename_all = "snake_case", tag = "key", content = "params")]
#[ts(export)]
pub enum ClientQuery {
    SysGetVolumes,
    SysGetLocations { id: String },
    LibExplorePath { path: String, limit: u32 },
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(rename_all = "snake_case", tag = "key", content = "data")]
#[ts(export)]
pub enum ClientResponse {
    SysGetVolumes(Vec<sys::volumes::Volume>),
    // SysGetLocations {
    //     locations: Vec<sys::locations::LocationData>,
    // },
}

pub struct Core {
    pub event_channel_sender: mpsc::Sender<ClientEvent>,
}
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("System error")]
    SysError(#[from] sys::SysError),
}

impl Core {
    pub async fn query(query: ClientQuery) -> Result<ClientResponse, CoreError> {
        println!("query: {:?}", query);
        let response = match query {
            ClientQuery::SysGetVolumes => ClientResponse::SysGetVolumes(sys::volumes::get()?),
            ClientQuery::SysGetLocations { id } => todo!(),
            ClientQuery::LibExplorePath { path, limit } => todo!(),
            // ClientQuery::SysGetLocations { id } => Ok(ClientResponse::SysGetLocations {
            //     locations: sys::locations::get(id)?,
            // }),
            // ClientQuery::LibExplorePath { path, limit } => Ok(ClientResponse::LibExplorePath {
            //     files: file::indexer::scan(path)?,
            // }),
        };
        Ok(response)
    }
}

// static configuration passed in by host application
#[derive(Serialize, Deserialize, Debug)]
pub struct CoreConfig {
    pub data_dir: std::path::PathBuf,
    pub primary_db: std::path::PathBuf,
    pub file_type_thumb_dir: std::path::PathBuf,
}

pub static CORE: OnceCell<Core> = OnceCell::new();

pub async fn core_send(event: ClientEvent) {
    println!("Core Event: {:?}", event);
    CORE.get()
        .unwrap()
        .event_channel_sender
        .send(event)
        .await
        .unwrap();
}

pub async fn core_send_stream<T: Stream<Item = ClientEvent>>(stream: T) {
    stream
        .for_each(|event| async {
            core_send(event).await;
        })
        .await;
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
    let mut client_config = ClientState::new(data_dir, "diamond-mastering-space-dragon").unwrap();
    // load from disk
    client_config
        .read_disk()
        .unwrap_or(println!("No client state found, creating new one..."));

    client_config.save();

    // begin asynchronous startup routines

    println!("Starting up... {:?}", client_config);
    if client_config.libraries.len() == 0 {
        match library::loader::create(None).await {
            Ok(library) => {
                println!("Created new library: {:?}", library);
            }
            Err(e) => {
                println!("Error creating library: {:?}", e);
            }
        }
    } else {
        for library in client_config.libraries.iter() {
            // init database for library
            match library::loader::load(&library.library_path, &library.library_id).await {
                Ok(library) => {
                    println!("Loaded library: {:?}", library);
                }
                Err(e) => {
                    println!("Error loading library: {:?}", e);
                }
            }
        }
    }

    // init client
    match client::create().await {
        Ok(_) => {
            println!("Spacedrive online");
        }
        Err(e) => {
            println!("Error initializing client: {:?}", e);
        }
    };
    // activate p2p listeners
    // p2p::listener::listen(None);

    // env_logger::builder()
    //     .filter_level(log::LevelFilter::Debug)
    //     .is_test(true)
    //     .init();

    event_receiver
}
