pub mod crypto;
pub mod db;
pub mod file;
pub mod library;
pub mod native;
pub mod state;
pub mod tx;
// pub mod p2p;
pub mod prisma;
pub mod util;

use futures::executor::block_on;
use futures::{stream::StreamExt, Stream};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::sync::mpsc;
use ts_rs::TS;

use state::client::ClientState;

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "type", content = "data")]
#[ts(export)]
pub enum ClientEvent {
    NewFileTypeThumb { file_id: u32, icon_created: bool },
    NewJobCreated { job_id: u32, progress: u8 },
    DatabaseDisconnected { reason: Option<String> },
}

pub struct Core {
    pub event_channel_sender: mpsc::Sender<ClientEvent>,
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
    match library::client::create().await {
        Ok(_) => {}
        Err(e) => {
            println!("Error initializing client: {:?}", e);
        }
    };
    // activate p2p listeners
    // p2p::listener::listen(None);
        
    println!("Spacedrive online");

    // env_logger::builder()
    //     .filter_level(log::LevelFilter::Debug)
    //     .is_test(true)
    //     .init();

    event_receiver
}
// println!("Spacedrive test");
// use crate::file::watcher::watch_dir;
// use std::{thread, time};
// watch_dir("/Users/jamie/Downloads/");
// let duration = time::Duration::from_secs(500);
// let now = time::Instant::now();
// thread::sleep(duration);
// assert!(now.elapsed() >= duration);

pub fn main() {
    // hello!
    println!("Hello, world!");
}
