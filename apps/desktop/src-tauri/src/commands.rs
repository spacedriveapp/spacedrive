use anyhow::Result;
use futures::{
  stream::{self, StreamExt},
  Stream,
};
use sdcorelib::db::connection::db_instance;
use sdcorelib::file::{indexer, retrieve, retrieve::Directory};
use sdcorelib::native;
use sdcorelib::AppConfig;
use serde::Serialize;
use std::fs;
use swift_rs::types::SRObjectArray;

pub fn reply<T: Serialize>(window: &tauri::Window, data: T) {
  let _message = window.emit("message", data).map_err(|e| println!("{}", e));
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
  Ok(&sdcorelib::APP_CONFIG.get().unwrap())
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

async fn stream_reply<T: Stream<Item = sdcorelib::ClientEvent>>(window: &tauri::Window, stream: T) {
  stream
    .for_each(|event| async { reply(window, event) })
    .await;
}

#[tauri::command(async)]
pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
  let thumbs = get_thumbs_for_directory_impl(path).await;

  stream_reply(&window, thumbs).await;

  Ok(())
}

pub async fn get_thumbs_for_directory_impl(
  path: &str,
) -> impl Stream<Item = sdcorelib::ClientEvent> {
  let dir = retrieve::get_dir_with_contents(&path).await.unwrap();

  stream::iter(dir.contents.into_iter()).filter_map(|file| async {
    let config = &sdcorelib::APP_CONFIG.get().unwrap();
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
      .unwrap();
    }

    if !existing {
      Some(sdcorelib::ClientEvent::NewFileTypeThumb {
        icon_created: true,
        file_id: file.id,
      })
    } else {
      None
    }
  })
}
