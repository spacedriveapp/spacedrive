use std::time::Instant;

use anyhow::Result;
use sdcorelib::db::connection::db_instance;
use sdcorelib::file::{indexer, retrieve, retrieve::Directory};
use sdcorelib::native;
use sdcorelib::AppConfig;
use serde::Serialize;
use std::fs;
use swift_rs::types::SRObjectArray;

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

  let files = indexer::scan(&path).await.map_err(|e| e.to_string())?;

  println!("file: {:?}", files);

  Ok(())
}

#[tauri::command(async)]
pub async fn get_files(path: String) -> Result<Directory, String> {
  Ok(retrieve::get_dir_with_contents(&path).await?)
}
#[tauri::command(async)]
pub async fn get_config() -> Result<&'static AppConfig, String> {
  Ok(&sdcorelib::EXTERNAL_CLIENT.get().unwrap().config)
}
#[tauri::command]
pub fn get_mounts() -> Result<SRObjectArray<native::methods::Mount>, String> {
  Ok(native::methods::get_mounts())
}

#[tauri::command(async)]
pub async fn test_scan() -> Result<(), String> {
  Ok(
    indexer::test_scan("/Users/jamie")
      .await
      .map_err(|e| e.to_string())?,
  )
}

// #[tauri::command(async)]
// pub async fn get_file_thumb(path: &str) -> Result<String, String> {
//   // let thumbnail_b64 = get_file_thumbnail_base64(path).to_string();

//   Ok(thumbnail_b64)
// }

// #[tauri::command(async)]
// pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
//   let config = CONFIG.get().unwrap();

//   let thumbnails = retrieve::get_thumbs_for_directory(path, &config).await?;

//   // ....
// }
#[tauri::command(async)]
pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
  let config = &sdcorelib::EXTERNAL_CLIENT.get().unwrap().config;
  let dir = retrieve::get_dir_with_contents(&path).await?;
  // iterate over directory contents
  for file in dir.contents.into_iter() {
    let now = Instant::now();
    let icon_name = format!(
      "{}.png",
      if file.is_dir {
        "folder".to_owned()
      } else {
        file.extension
      }
    );
    let icon_path = config.file_type_thumb_dir.join(icon_name);
    // extract metadata from file
    let existing = fs::metadata(&icon_path).is_ok();
    // write thumbnail only if
    if !existing {
      // call swift to get thumbnail data
      let thumbnail_b64 =
        sdcorelib::native::methods::get_file_thumbnail_base64(&file.uri).to_string();
      fs::write(
        &icon_path,
        base64::decode(thumbnail_b64).unwrap_or_default(),
      )
      .map_err(|_| "thumb_cache_failure")?;
    }
    println!("cached thumb {:?} in {:?}", file.id, now.elapsed());

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
