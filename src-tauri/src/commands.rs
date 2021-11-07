use crate::app::config;
use crate::db::connection::db_instance;
use crate::filesystem;
use crate::filesystem::retrieve::Directory;
use crate::swift;
use crate::swift::get_file_thumbnail_base64;
use anyhow::Result;
use base64;
use serde::Serialize;
use swift_rs::types::{SRObjectArray};
use std::fs;
use std::time::Instant;
use walkdir::WalkDir;

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
  pub file_id: u32,
  pub icon_created: bool,
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
#[tauri::command(async)]
pub async fn get_config() -> Result<config::AppConfig, String> {
  Ok(config::get_config())
}
#[tauri::command]
pub fn get_mounts() -> Result<SRObjectArray<swift::Mount>, String> {
  Ok(swift::get_mounts())
}

#[tauri::command]
pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
  let config = config::get_config();
  let dir = filesystem::retrieve::get_dir_with_contents(&path).await?;
  for file in dir.contents.into_iter() {
    // let now = Instant::now();
    let icon_name = format!(
      "{}.png",
      if file.is_dir {
        "folder".to_owned()
      } else {
        file.extension
      }
    );

    let icon_path = config.file_type_thumb_dir.join(icon_name);

    let existing = fs::metadata(&icon_path).is_ok();

    if !existing {
      let thumbnail_b64 = get_file_thumbnail_base64(&file.uri).to_string();
      fs::write(&icon_path, base64::decode(thumbnail_b64).unwrap()).expect("Unable to write file")
    }

    // println!("got thumb {:?} in {:?}", file.id, now.elapsed());
    if !existing {
      reply(
        &window,
        GlobalEventKind::FileTypeThumb,
        GenFileTypeIconsResponse {
          icon_created: true,
          file_id: file.id,
        },
      )
    }
  }

  Ok(())
}

#[tauri::command(async)]
pub async fn test_scan() -> Result<(), String> {
  let mut count: u32 = 0;
  for entry in WalkDir::new("/Users/jamie")
    .into_iter()
    .filter_map(|e| e.ok())
  {
    let child_path = entry.path().to_str().unwrap();
    count = count + 1;
    println!("Reading file from dir {:?}", child_path);
  }
  println!("files found {}", count);

  Ok(())
}
