use std::time::{Duration, Instant};

use sdcore::{ClientCommand, ClientQuery, Core, CoreController, CoreEvent, CoreResponse};
use tauri::api::path;
use tauri::Manager;
mod menu;

use window_shadows::set_shadow;

#[tauri::command(async)]
async fn client_query_transport(
  core: tauri::State<'_, CoreController>,
  data: ClientQuery,
) -> Result<CoreResponse, String> {
  match core.query(data).await {
    Ok(response) => Ok(response),
    Err(err) => {
      println!("query error: {:?}", err);
      Err(err.to_string())
    }
  }
}

#[tauri::command(async)]
async fn client_command_transport(
  core: tauri::State<'_, CoreController>,
  data: ClientCommand,
) -> Result<CoreResponse, String> {
  match core.command(data).await {
    Ok(response) => Ok(response),
    Err(err) => {
      println!("command error: {:?}", err);
      Err(err.to_string())
    }
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

      app.windows().iter().for_each(|(_, window)| {
        set_shadow(&window, true).unwrap_or(());

        window.start_dragging().unwrap_or(());
      });

      // core event transport
      tokio::spawn(async move {
        let mut last = Instant::now();
        // handle stream output
        while let Some(event) = event_receiver.recv().await {
          match event {
            CoreEvent::InvalidateQueryDebounced(_) => {
              let current = Instant::now();
              if current.duration_since(last) > Duration::from_millis(1000 / 60) {
                last = current;
                app.emit_all("core_event", &event).unwrap();
              }
            }
            event => {
              app.emit_all("core_event", &event).unwrap();
            }
          }
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
