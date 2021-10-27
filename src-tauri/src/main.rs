#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]
mod app;
mod commands;
mod crypto;
mod db;
mod filesystem;
mod swift;
mod util;
use crate::app::menu;
use futures::executor::block_on;

fn main() {
  // env_logger::builder()
  //   .filter_level(log::LevelFilter::Debug)
  //   .is_test(true)
  //   .init();

  // create primary data base if not exists
  block_on(db::connection::create_primary_db()).unwrap();
  // init filesystem and create library if missing
  block_on(filesystem::init::init_library()).unwrap();

  // block_on(filesystem::device::discover_storage_devices()).unwrap();

  tauri::Builder::default()
    .setup(|_app| {
      // let main_window = app.get_window("main").unwrap();
      // // would need to emit this elsewhere in my Rust code
      // main_window.emit("my-event", "payload");
      Ok(())
    })
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![
      commands::get_config,
      commands::scan_dir,
      commands::get_mounts,
      commands::get_files,
      commands::get_file_thumb,
      commands::test_scan,
      commands::get_thumbs_for_directory
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
