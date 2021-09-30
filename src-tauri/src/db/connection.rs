use crate::app::config;
use rusqlite;
use tauri::InvokeError;

pub fn get_connection() -> Result<rusqlite::Connection, InvokeError> {
  let config = config::get_config();

  rusqlite::Connection::open(config.primary_db)
    .map_err(|error| InvokeError::from("database_connection_failure"))
}
