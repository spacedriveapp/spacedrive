use sdcorelib::{ClientCommand, ClientQuery, Core, CoreController, CoreResponse};
use tauri::api::path;
use tauri::Manager;
// use tauri_plugin_shadows::Shadows;
// mod commands;
mod menu;

#[tauri::command(async)]
async fn client_query_transport(
  core: tauri::State<'_, CoreController>,
  data: ClientQuery,
) -> Result<CoreResponse, String> {
  match core.query(data).await {
    Ok(response) => Ok(response),
    Err(err) => Err(err.to_string()),
  }
}

#[tauri::command(async)]
async fn client_command_transport(
  core: tauri::State<'_, CoreController>,
  data: ClientCommand,
) -> Result<CoreResponse, String> {
  match core.command(data).await {
    Ok(response) => Ok(response),
    Err(err) => Err(err.to_string()),
  }
}

#[tokio::main]
async fn main() {
  let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
  // create an instance of the core
  let (mut core, mut event_receiver) = Core::new(data_dir).await;
  // run startup tasks
  core.initializer().await;
  // extract the core controller
  let controller = core.get_controller();
  // throw the core into a dedicated thread
  tokio::spawn(async move {
    core.start().await;
  });
  // create tauri app
  tauri::Builder::default()
    // pass controller to the tauri state manager
    .manage(controller)
    .setup(|app| {
      let app = app.handle();
      // core event transport
      tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
          app.emit_all("core_event", &event).unwrap();
        }
      });

      Ok(())
    })
    .on_menu_event(|event| menu::handle_menu_event(event))
    .invoke_handler(tauri::generate_handler![
      client_query_transport,
      client_command_transport,
    ])
    .menu(menu::get_menu())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
