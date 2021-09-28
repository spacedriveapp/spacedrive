use rusqlite;
// use tauri::api::path;
use crate::app::config;

pub fn create_connection() -> Result<rusqlite::Connection, rusqlite::Error> {
  let config = config::get_config();

  rusqlite::Connection::open(config.primary_db)
}
