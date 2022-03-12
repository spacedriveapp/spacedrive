use anyhow::Result;
use sdcorelib::{
  file::{indexer, retrieve, retrieve::Directory, watcher::watch_dir},
  state::{client, client::ClientState},
  sys,
  sys::{volumes, volumes::Volume},
  ClientQuery, ClientResponse, Core,
};

#[tauri::command(async)]
pub async fn client_query_transport(data: ClientQuery) -> Result<ClientResponse, String> {
  match Core::query(data).await {
    Ok(response) => Ok(response),
    Err(err) => Err(err.to_string()),
  }
}

#[tauri::command(async)]
pub async fn scan_dir(path: String) -> Result<(), String> {
  let files = indexer::scan(&path).await.map_err(|e| e.to_string());

  println!("file: {:?}", files);

  Ok(())
}

#[tauri::command(async)]
pub async fn get_files(path: String) -> Result<Directory, String> {
  Ok(retrieve::get_dir_with_contents(&path).await?)
}

#[tauri::command]
pub fn get_config() -> ClientState {
  client::get()
}

#[tauri::command]
pub fn get_mounts() -> Result<Vec<Volume>, String> {
  Ok(volumes::get().unwrap())
}

#[tauri::command(async)]
pub async fn test_scan() -> Result<(), String> {
  Ok(
    indexer::test_scan("/Users/jamie")
      .await
      .map_err(|e| e.to_string())?,
  )
}

#[tauri::command]
pub async fn start_watcher(path: &str) -> Result<(), String> {
  println!("starting watcher for: {:?}", path);
  watch_dir(&path);

  Ok(())
}

#[tauri::command]
pub async fn create_location(path: &str) -> Result<(), String> {
  let _location = sys::locations::create_location(path);
  Ok(())
}
