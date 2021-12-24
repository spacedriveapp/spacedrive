#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]
mod app;
mod commands;
use crate::app::menu;

fn main() {
  tauri::Builder::default()
    .setup(|_app| Ok(()))
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
