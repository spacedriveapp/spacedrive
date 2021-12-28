use futures::{stream::StreamExt, Stream};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::sync::mpsc;
use ts_rs::TS;

pub mod crypto;
pub mod db;
pub mod file;
pub mod native;
pub mod util;
use futures::executor::block_on;

// static configuration
#[derive(Serialize, Deserialize, Debug)]
pub struct CoreConfig {
    pub data_dir: std::path::PathBuf,
    pub primary_db: std::path::PathBuf,
    pub file_type_thumb_dir: std::path::PathBuf,
}

pub struct Core {
    pub config: CoreConfig,
    pub event_channel_sender: mpsc::Sender<ClientEvent>,
}

// represents an event this library can emit
#[derive(Serialize, Deserialize, Debug, TS)]
#[serde(tag = "type", content = "data")]
#[ts(export)]
pub enum ClientEvent {
    NewFileTypeThumb { file_id: u32, icon_created: bool },
}

pub static CORE: OnceCell<Core> = OnceCell::new();

pub fn get_core_config() -> &'static CoreConfig {
    &CORE.get().unwrap().config
}

pub async fn core_send(event: ClientEvent) {
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

pub fn configure(mut data_dir: std::path::PathBuf) -> mpsc::Receiver<ClientEvent> {
    data_dir = data_dir.join("spacedrive");

    let (event_sender, event_receiver) = mpsc::channel(100);

    let config = CoreConfig {
        data_dir: data_dir.clone(),
        primary_db: data_dir.clone().join("primary.db3"),
        file_type_thumb_dir: data_dir.clone().join("file_icons"),
    };

    CORE.set(Core {
        config: config,
        event_channel_sender: event_sender,
    });

    // create the data directories if not present
    fs::create_dir_all(&get_core_config().data_dir).unwrap();
    fs::create_dir_all(&get_core_config().file_type_thumb_dir).unwrap();

    // create primary data base if not exists
    block_on(db::connection::create_primary_db()).unwrap();
    // init filesystem and create library if missing
    block_on(file::init::init_library()).unwrap();

    println!("Spacedrive daemon online");

    event_receiver
}

// pub static MAIN_WINDOW: OnceCell<> = OnceCell::new();
// // handler to pass "callback" to OneCell intercepting the commands
// pub fn emit(kind: &str, data: &str) {
//     let _message = MAIN_WINDOW
//         .get()
//         .unwrap()
//         .emit(kind, data)
//         .map_err(|e| println!("{}", e));
// }

fn main() {
    // hello!
    println!("Hello, world!");
}
