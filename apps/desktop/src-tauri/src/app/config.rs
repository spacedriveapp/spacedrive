use serde::Serialize;
use std::fs;
use tauri::api::path;

#[derive(Serialize)]
pub struct AppConfig {
  pub primary_db: std::path::PathBuf,
  pub data_dir: std::path::PathBuf,
  pub file_type_thumb_dir: std::path::PathBuf,
}

// returns the app config struct with complete values
pub fn get_config() -> AppConfig {
  let app_name = "SpaceDrive";
  let data_dir = path::data_dir()
    .unwrap_or(std::path::PathBuf::from("./"))
    .join(app_name);
  let file_type_thumb_dir = data_dir.join("file_icons");

  // create the data directory if not exists
  fs::create_dir_all(&data_dir).unwrap();
  fs::create_dir_all(&file_type_thumb_dir).unwrap();

  AppConfig {
    primary_db: data_dir.join("primary.db3"),
    data_dir,
    file_type_thumb_dir,
  }
}
