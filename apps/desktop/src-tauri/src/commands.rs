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

pub fn reply<T: Serialize>(window: &tauri::Window, kind: GlobalEventKind, data: T) {
  let _message = window
    .emit("message", GlobalEvent { kind, data })
    .map_err(|e| println!("{}", e));
}

#[tauri::command(async)]
pub async fn scan_dir(window: tauri::Window, path: String) -> Result<(), String> {
  Ok(())
}
