#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod app;
mod commands;
mod crypto;
mod db;
mod filesystem;
mod util;
use crate::app::menu;

fn main() {
  let connection = db::init::create_connection();

  println!("primary database connected {:?}", connection);

  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![commands::read_file_command])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
