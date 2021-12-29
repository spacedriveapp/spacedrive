use anyhow::Result;
use sdcorelib::{
  core_send_stream,
  db::connection::db_instance,
  file::{icon, indexer, retrieve, retrieve::Directory, watcher::watch_dir},
  get_core_config, native, CoreConfig,
};
use swift_rs::types::SRObjectArray;

#[tauri::command(async)]
pub async fn scan_dir(path: String) -> Result<(), String> {
  db_instance().await?;

  let files = indexer::scan(&path).await.map_err(|e| e.to_string())?;

  println!("file: {:?}", files);

  Ok(())
}

#[tauri::command(async)]
pub async fn get_files(path: String) -> Result<Directory, String> {
  Ok(retrieve::get_dir_with_contents(&path).await?)
}

#[tauri::command(async)]
pub async fn get_config() -> Result<&'static CoreConfig, String> {
  Ok(get_core_config())
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

#[tauri::command(async)]
pub async fn get_thumbs_for_directory(path: &str) -> Result<(), String> {
  core_send_stream(icon::get_thumbs_for_directory(path).await).await;

  Ok(())
}
#[tauri::command]
pub async fn start_watcher(path: &str) -> Result<(), String> {
  println!("starting watcher for: {:?}", path);
  watch_dir(&path);

  Ok(())
}
