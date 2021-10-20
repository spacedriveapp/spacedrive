use crate::db::entity::file;
use crate::filesystem::retrieve::Directory;
use crate::swift::get_file_thumbnail_base64;
use crate::{db, filesystem};
use anyhow::Result;
use once_cell::sync::OnceCell;
use sea_orm::ColumnTrait;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter};

pub static DB_INSTANCE: OnceCell<DatabaseConnection> = OnceCell::new();

async fn db_instance() -> Result<&'static DatabaseConnection, String> {
  if DB_INSTANCE.get().is_none() {
    let db = db::connection::get_connection()
      .await
      .map_err(|e| e.to_string())?;
    DB_INSTANCE.set(db).unwrap_or_default();
    Ok(DB_INSTANCE.get().unwrap())
  } else {
    Ok(DB_INSTANCE.get().unwrap())
  }
}

#[tauri::command(async)]
pub async fn scan_dir(path: String) -> Result<(), String> {
  db_instance().await?;

  let files = filesystem::indexer::scan(&path)
    .await
    .map_err(|e| e.to_string())?;

  println!("file: {:?}", files);

  Ok(())
}
#[tauri::command(async)]
pub async fn get_file_thumb(path: &str) -> Result<String, String> {
  let thumbnail_b46 = get_file_thumbnail_base64(path).to_string();

  Ok(thumbnail_b46)
}

#[tauri::command(async)]
pub async fn get_files(path: String) -> Result<Directory, String> {
  let connection = db_instance().await?;

  println!("getting files... {:?}", &path);

  let directories = file::Entity::find()
    .filter(file::Column::Uri.eq(path))
    .all(connection)
    .await
    .map_err(|e| e.to_string())?;

  if directories.is_empty() {
    return Err("fuk".to_owned());
  }

  let directory = &directories[0];

  let files = file::Entity::find()
    .filter(file::Column::ParentId.eq(directory.id))
    .all(connection)
    .await
    .map_err(|e| e.to_string())?;

  Ok(Directory {
    directory: directory.clone(),
    contents: files,
  })
}
