use tauri::{AboutMetadata, CustomMenuItem, Menu, MenuItem, Submenu, WindowMenuEvent, Wry};

pub(crate) fn get_menu() -> Menu {
  // let quit = CustomMenuItem::new("quit".to_string(), "Quit");
  // let close = CustomMenuItem::new("close".to_string(), "Close");
  // let jeff = CustomMenuItem::new("jeff".to_string(), "Jeff");
  // let submenu = Submenu::new(
  //   "File",
  //   Menu::new().add_item(quit).add_item(close).add_item(jeff),
  // );
  let spacedrive = Submenu::new(
    "Spacedrive",
    Menu::new()
      .add_native_item(MenuItem::About(
        "Spacedrive".to_string(),
        AboutMetadata::new(),
      )) // TODO: fill out about metadata
      .add_native_item(MenuItem::Separator)
      .add_native_item(MenuItem::Services)
      .add_native_item(MenuItem::Separator)
      .add_native_item(MenuItem::Hide)
      .add_native_item(MenuItem::HideOthers)
      .add_native_item(MenuItem::ShowAll)
      .add_native_item(MenuItem::Separator)
      .add_native_item(MenuItem::Quit),
  );

  let file = Submenu::new(
    "File",
    Menu::new()
      .add_item(CustomMenuItem::new("quit".to_string(), "Quit"))
      .add_item(CustomMenuItem::new("close".to_string(), "Close")),
  );
  let edit = Submenu::new(
    "Edit",
    Menu::new()
      .add_item(CustomMenuItem::new("jeff".to_string(), "Copy"))
      .add_item(CustomMenuItem::new("jeffd".to_string(), "Paste")),
  );
  let view = Submenu::new(
    "View",
    Menu::new()
      .add_item(CustomMenuItem::new(
        "command_pallete".to_string(),
        "Command Pallete",
      ))
      .add_item(CustomMenuItem::new("jeffd".to_string(), "Layout")),
  );

  let menu = Menu::new()
    .add_submenu(spacedrive)
    .add_submenu(file)
    .add_submenu(edit)
    .add_submenu(view);

  menu
}

pub(crate) fn handle_menu_event(event: WindowMenuEvent<Wry>) {
  match event.menu_item_id() {
    "quit" => {
      std::process::exit(0);
    }
    "close" => {
      event.window().close().unwrap();
    }
    _ => {}
  }
}
