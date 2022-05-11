use std::env::consts;
use std::time::{Duration, Instant};

use cocoa::appkit::{NSWindow, NSWindowStyleMask};
use cocoa::base::nil;
use sdcore::{ClientCommand, ClientQuery, CoreController, CoreEvent, CoreResponse, Node};
use tauri::api::path;
use tauri::Manager;
use tauri::{Runtime, Window};
mod menu;

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

pub trait WindowExt {
  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool);
}

impl<R: Runtime> WindowExt for Window<R> {
  #[cfg(target_os = "macos")]
  fn set_transparent_titlebar(&self, transparent: bool) {
    use cocoa::{
      appkit::{NSApplication, NSToolbar, NSWindowTitleVisibility},
      foundation::NSString,
    };

    unsafe {
      let id = self.ns_window().unwrap() as cocoa::base::id;

      let mut style_mask = id.styleMask();
      style_mask.set(
        NSWindowStyleMask::NSFullSizeContentViewWindowMask
          | NSWindowStyleMask::NSUnifiedTitleAndToolbarWindowMask,
        transparent,
      );
      id.setStyleMask_(style_mask);

      // TODO: figure out if this is how to correctly hide the toolbar in full screen
      // and if so, figure out why tf it panics:

      // let mut presentation_options = id.presentationOptions_();
      // presentation_options.set(
      //   NSApplicationPresentationOptions::NSApplicationPresentationAutoHideToolbar,
      //   transparent,
      // );
      // id.setPresentationOptions_(presentation_options);

      let toolbar = NSToolbar::alloc(nil).initWithIdentifier_(NSString::alloc(nil).init_str("wat"));
      toolbar.setShowsBaselineSeparator_(false);
      id.setToolbar_(toolbar);

      id.setTitleVisibility_(if transparent {
        NSWindowTitleVisibility::NSWindowTitleHidden
      } else {
        NSWindowTitleVisibility::NSWindowTitleVisible
      });

      id.setTitlebarAppearsTransparent_(if transparent {
        cocoa::base::YES
      } else {
        cocoa::base::NO
      });
    }
  }
}

#[tokio::main]
async fn main() {
  let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
  // create an instance of the core
  let (mut node, mut event_receiver) = Node::new(data_dir).await;
  // run startup tasks
  node.initializer().await;
  // extract the node controller
  let controller = node.get_controller();
  // throw the node into a dedicated thread
  tokio::spawn(async move {
    node.start().await;
  });
  // create tauri app
  tauri::Builder::default()
    // pass controller to the tauri state manager
    .manage(controller)
    .setup(|app| {
      let app = app.handle();

      #[cfg(not(target_os = "linux"))]
      {
        app.windows().iter().for_each(|(_, window)| {
          window_shadows::set_shadow(&window, true).unwrap_or(());

          if consts::OS == "windows" {
            let _ = window.set_decorations(true);
          }
        });
      }

      #[cfg(target_os = "macos")]
      {
        let win = app.get_window("main").unwrap();
        win.set_transparent_titlebar(true);
      }

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
