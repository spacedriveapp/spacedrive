use sdcorelib::Core;
use tauri::api::path;
use tauri::Manager;
// use tauri_plugin_shadows::Shadows;

mod commands;
mod menu;

#[tokio::main]
async fn main() {
  let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));

  let mut core = Core::new(data_dir).await;

  tauri::Builder::default()
    .setup(|app| {
      let app = app.handle();

      tauri::async_runtime::spawn(async move {
        while let Some(event) = core.event_receiver.recv().await {
          app.emit_all("core_event", &event).unwrap();
        }
      });

      Ok(())
    })
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![
      commands::client_query_transport,
      commands::scan_dir,
      commands::create_location,
      commands::get_files,
      commands::get_config,
      commands::get_mounts,
      commands::test_scan,
      commands::start_watcher,
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
