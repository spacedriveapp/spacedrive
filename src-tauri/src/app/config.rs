use std::fs;
use tauri::api::path;

pub struct AppConfig {
  pub primary_db: std::path::PathBuf,
  pub data_dir: std::path::PathBuf,
}

// returns the app config struct with complete values
pub fn get_config() -> AppConfig {
  let app_name = "spacedrive";
  let data_dir = path::data_dir()
    .unwrap_or(std::path::PathBuf::from("./"))
    .join(app_name);

  // create the data directory if not exists
  fs::create_dir_all(&data_dir).unwrap();

  AppConfig {
    primary_db: data_dir.join("primary.db3"),
    data_dir,
  }
}
