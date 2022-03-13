use once_cell::sync::OnceCell;
use sdcorelib::{ClientCommand, ClientQuery, Core, CoreResponse};
use tauri::api::path;
use tauri::Manager;
// use tauri_plugin_shadows::Shadows;

mod commands;
mod menu;

pub static CORE: OnceCell<Core> = OnceCell::new();

#[tauri::command(async)]
async fn client_query_transport(data: ClientQuery) -> Result<CoreResponse, String> {
  match CORE.get().unwrap().query(data).await {
    Ok(response) => Ok(response),
    Err(err) => Err(err.to_string()),
  }
}

#[tauri::command(async)]
async fn client_command_transport(data: ClientCommand) -> Result<CoreResponse, String> {
  match CORE.get().unwrap().command(data).await {
    Ok(response) => Ok(response),
    Err(err) => Err(err.to_string()),
  }
}

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
      client_query_transport,
      client_command_transport,
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
