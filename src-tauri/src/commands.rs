use crate::db::connection::db_instance;
use crate::filesystem;
use crate::filesystem::retrieve::Directory;
use crate::swift::get_file_thumbnail_base64;
use anyhow::Result;

use serde::Serialize;

#[derive(Serialize)]
pub enum GlobalEventKind {
  FileTypeThumb,
}

#[derive(Serialize)]
pub struct GlobalEvent<T> {
  pub kind: GlobalEventKind,
  pub data: T,
}
#[derive(Serialize)]
pub struct GenFileTypeIconsResponse {
  pub thumbnail_b64: String,
  pub file_id: u32,
}

pub fn reply<T: Serialize>(window: &tauri::Window, kind: GlobalEventKind, data: T) {
  let _message = window
    .emit("message", GlobalEvent { kind, data })
    .map_err(|e| println!("{}", e));
}

#[tauri::command(async)]
pub async fn scan_dir(window: tauri::Window, path: String) -> Result<(), String> {
  db_instance().await?;

  // reply(&window, GlobalEventKind::JEFF, "jeff");

  let files = filesystem::indexer::scan(&path)
    .await
    .map_err(|e| e.to_string())?;

  println!("file: {:?}", files);

  Ok(())
}
#[tauri::command(async)]
pub async fn get_file_thumb(path: &str) -> Result<String, String> {
  let thumbnail_b64 = get_file_thumbnail_base64(path).to_string();

  Ok(thumbnail_b64)
}

#[tauri::command(async)]
pub async fn get_files(path: String) -> Result<Directory, String> {
  Ok(filesystem::retrieve::get_dir_with_contents(&path).await?)
}

#[tauri::command]
pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
  let dir = filesystem::retrieve::get_dir_with_contents(&path).await?;
  for file in dir.contents.into_iter() {
    let thumbnail_b64 = get_file_thumbnail_base64(&file.uri).to_string();
    println!("getting thumb: {:?}", file.id);
    reply(
      &window,
      GlobalEventKind::FileTypeThumb,
      GenFileTypeIconsResponse {
        thumbnail_b64,
        file_id: file.id,
      },
    )
  }

  Ok(())
}
