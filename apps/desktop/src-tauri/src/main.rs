mod commands;
mod menu;
use sdcorelib;
use tauri::api::path;
use tauri::Manager;
use tauri_plugin_shadows::Shadows;

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
      let mut core_receiver = sdcorelib::configure(data_dir);

      let app = app.handle();

      let window = app.get_window("main").unwrap();
//       window.set_shadow(true);

      tauri::async_runtime::spawn(async move {
        while let Some(event) = core_receiver.recv().await {
          app.emit_all("core_event", &event).unwrap();
        }
      });

      Ok(())
    })
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![
      commands::scan_dir,
      commands::get_files,
      commands::get_config,
      commands::get_mounts,
      commands::test_scan,
      commands::get_thumbs_for_directory,
      commands::start_watcher,
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
