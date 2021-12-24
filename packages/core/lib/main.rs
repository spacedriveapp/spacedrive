use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

mod crypto;
mod db;
mod file;
mod util;

// static configuration
#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub data_dir: std::path::PathBuf,
    pub primary_db: std::path::PathBuf,
    pub file_type_thumb_dir: std::path::PathBuf,
}
pub static CONFIG: OnceCell<AppConfig> = OnceCell::new();

pub fn configure(data_dir: std::path::PathBuf) {
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
