mod commands;
mod menu;

use sdcorelib;
use tauri::api::path;

fn main() {
  tauri::Builder::default()
    .setup(|_app| {
      let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
      sdcorelib::configure(data_dir);
      Ok(())
    })
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
