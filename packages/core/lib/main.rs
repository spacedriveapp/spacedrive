use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

pub mod crypto;
pub mod db;
pub mod file;
pub mod native;
pub mod util;

// static configuration
#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub data_dir: std::path::PathBuf,
    pub primary_db: std::path::PathBuf,
    pub file_type_thumb_dir: std::path::PathBuf,
}
pub static CONFIG: OnceCell<AppConfig> = OnceCell::new();

pub fn configure(mut data_dir: std::path::PathBuf) {
    data_dir = data_dir.join("spacedrive");

    let config = AppConfig {
        data_dir: data_dir.clone(),
        primary_db: data_dir.clone().join("primary.db3"),
        file_type_thumb_dir: data_dir.clone().join("file_icons"),
    };
    CONFIG.set(config).unwrap();
}

fn main() {
    // hello!
    println!("Hello, world!");
}
