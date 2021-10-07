use crate::db::entity::file;
use crate::filesystem::{init, reader};
use crate::{db, filesystem};
use anyhow::Result;
use once_cell::sync::OnceCell;
use sea_orm::{DatabaseConnection, EntityTrait};

pub static DB_INSTANCE: OnceCell<DatabaseConnection> = OnceCell::new();

async fn db_instance() -> Result<&'static DatabaseConnection> {
  if DB_INSTANCE.get().is_none() {
    let db = db::connection::get_connection().await?;
    DB_INSTANCE.set(db).unwrap_or_default();
    Ok(DB_INSTANCE.get().unwrap())
  } else {
    Ok(DB_INSTANCE.get().unwrap())
  }
}

#[tauri::command(async)]
pub async fn scan_dir(path: &str) -> Result<(), String> {
  db_instance().await.map_err(|e| e.to_string())?;

  let directories = filesystem::explorer::scan(path)
    .await
    .map_err(|e| e.to_string())?;

  println!("file: {:?}", directories);

  Ok(())
}

#[tauri::command(async)]
pub async fn get_files() -> Result<Vec<file::Model>, String> {
  let connection = db_instance().await.map_err(|e| e.to_string())?;

  let files = file::Entity::find()
    .all(connection)
    .await
    .map_err(|e| e.to_string())?;

  println!("files found, {:?}", files.len());

  Ok(files[..100].to_vec())
}

// #[tauri::command(async)]
// pub async fn generate_buffer_checksum(path: &str) -> Result<File, InvokeError> {
//   let mut file = file::read_file(path)
//     .await
//     .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;

//   // file.buffer_checksum = Some(checksum::create_hash(&file.uri).await.unwrap());
//   Ok(file)
// }
