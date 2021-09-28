#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod app;
mod crypto;
mod db;
mod filesystem;
mod util;
use crate::app::menu;

#[derive(serde::Serialize)]
struct CustomResponse {
  message: String,
}

#[tauri::command]
async fn fn_exposed_to_js(window: tauri::Window) -> Result<CustomResponse, String> {
  println!("Called from window {}", window.label());
  Ok(CustomResponse {
    message: "Hello from rust!".to_string(),
  })
}

fn main() {
  let connection = db::init::create_connection();
  // let hash = filestuff::create_hash("/Users/jamie/Desktop/jeff.MP4");
  println!("jeff {:?}", connection);

  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      fn_exposed_to_js,
      filesystem::file::read_file_command
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
