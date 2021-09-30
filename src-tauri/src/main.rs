#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

mod app;
// mod commands;
mod crypto;
mod db;
mod filesystem;
mod util;
use crate::app::menu;
use futures::executor::block_on;

fn main() {
  let connection = db::connection::get_connection();
  let primary_db = block_on(connection).unwrap();
  // db::migrate::run_migrations(connection);

  println!("primary database connected {:?}", primary_db);

  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      // commands::read_file_command,
      // commands::generate_buffer_checksum
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
