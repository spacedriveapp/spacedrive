use std::str::FromStr;

use serde::Deserialize;
use specta::Type;
use tauri::{
	menu::{Menu, MenuItemKind},
	AppHandle, Emitter, Manager, Wry,
};
use tracing::error;

#[derive(
	Debug, Clone, Copy, Type, Deserialize, strum::EnumString, strum::AsRefStr, strum::Display,
)]
pub enum MenuEvent {
	NewLibrary,
	NewFile,
	NewDirectory,
	AddLocation,
	OpenOverview,
	OpenSearch,
	OpenSettings,
	ReloadExplorer,
	SetLayoutGrid,
	SetLayoutList,
	SetLayoutMedia,
	ToggleDeveloperTools,
	NewWindow,
	ReloadWebview,
	Copy,
	Cut,
	Paste,
	Duplicate,
	SelectAll,
}

/// Menu items which require a library to be open to use.
/// They will be disabled/enabled automatically.
const LIBRARY_LOCKED_MENU_IDS: &[MenuEvent] = &[
	MenuEvent::NewWindow,
	MenuEvent::OpenOverview,
	MenuEvent::OpenSearch,
	MenuEvent::OpenSettings,
	MenuEvent::ReloadExplorer,
	MenuEvent::SetLayoutGrid,
	MenuEvent::SetLayoutList,
	MenuEvent::SetLayoutMedia,
	MenuEvent::NewFile,
	MenuEvent::NewDirectory,
	MenuEvent::NewLibrary,
	MenuEvent::AddLocation,
];

pub fn setup_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
	app.on_menu_event(move |app, event| {
		if let Ok(event) = MenuEvent::from_str(&event.id().0) {
			handle_menu_event(event, app);
		} else {
			println!("Unknown menu event: {}", event.id().0);
		}
	});

	#[cfg(not(target_os = "macos"))]
	{
		Menu::new(app)
	}
	#[cfg(target_os = "macos")]
	{
		use tauri::menu::{AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder};

		let app_menu = SubmenuBuilder::new(app, "Spacedrive")
			.about(Some(
				AboutMetadataBuilder::new()
					.authors(Some(vec!["Spacedrive Technology Inc.".to_string()]))
					.license(Some(env!("CARGO_PKG_VERSION")))
					.version(Some(env!("CARGO_PKG_VERSION")))
					.website(Some("https://spacedrive.com/"))
					.website_label(Some("Spacedrive.com"))
					.build(),
			))
			.separator()
			.item(&MenuItemBuilder::with_id(MenuEvent::NewLibrary, "New Library").build(app)?)
			// .item(
			// 	&SubmenuBuilder::new(app, "Libraries")
			// 		// TODO: Implement this
			// 		.items(&[])
			// 		.build()?,
			// )
			.separator()
			.hide()
			.hide_others()
			.show_all()
			.separator()
			.quit()
			.build()?;

		// TODO: Re-enable these when they are implemented, and doesn't stop duplicates.
		// let file_menu = SubmenuBuilder::new(app, "File")
		// 	.item(
		// 		&MenuItemBuilder::with_id(MenuEvent::NewFile, "New File")
		// 			.accelerator("CmdOrCtrl+N")
		// 			.build(app)?,
		// 	)
		// 	.item(
		// 		&MenuItemBuilder::with_id(MenuEvent::NewDirectory, "New Directory")
		// 			.accelerator("CmdOrCtrl+D")
		// 			.build(app)?,
		// 	)
		// 	.item(
		// 		&MenuItemBuilder::with_id(MenuEvent::AddLocation, "Add Location")
		// 			// .accelerator("") // TODO
		// 			.build(app)?,
		// 	)
		// 	.build()?;

		let edit_menu = SubmenuBuilder::new(app, "Edit")
			// .item(
			// 	&MenuItemBuilder::with_id(MenuEvent::Copy, "Copy")
			// 		.accelerator("CmdOrCtrl+C")
			// 		.build(app)?,
			// )
			// .item(
			// 	&MenuItemBuilder::with_id(MenuEvent::Cut, "Cut")
			// 		.accelerator("CmdOrCtrl+X")
			// 		.build(app)?,
			// )
			// .item(
			// 	&MenuItemBuilder::with_id(MenuEvent::Paste, "Paste")
			// 		.accelerator("CmdOrCtrl+V")
			// 		.build(app)?,
			// )
			// .item(
			// 	&MenuItemBuilder::with_id(MenuEvent::Duplicate, "Duplicate")
			// 		.accelerator("CmdOrCtrl+D")
			// 		.build(app)?,
			// )
			.select_all()
			.undo()
			.redo()
			.build()?;

		let view_menu = SubmenuBuilder::new(app, "View")
			.item(
				&MenuItemBuilder::with_id(MenuEvent::OpenOverview, "Open Overview")
					.accelerator("CmdOrCtrl+.")
					.build(app)?,
			)
			.item(
				&MenuItemBuilder::with_id(MenuEvent::OpenSearch, "Search")
					.accelerator("CmdOrCtrl+F")
					.build(app)?,
			)
			.item(
				&MenuItemBuilder::with_id(MenuEvent::OpenSettings, "Settings")
					.accelerator("CmdOrCtrl+Comma")
					.build(app)?,
			)
			.item(
				&MenuItemBuilder::with_id(MenuEvent::ReloadExplorer, "Open Explorer")
					.accelerator("CmdOrCtrl+R")
					.build(app)?,
			)
			.item(
				&SubmenuBuilder::new(app, "Layout")
					.item(
						&MenuItemBuilder::with_id(MenuEvent::SetLayoutGrid, "Grid (Default)")
							// .accelerator("") // TODO
							.build(app)?,
					)
					.item(
						&MenuItemBuilder::with_id(MenuEvent::SetLayoutList, "List")
							// .accelerator("") // TODO
							.build(app)?,
					)
					.item(
						&MenuItemBuilder::with_id(MenuEvent::SetLayoutMedia, "Media")
							// .accelerator("") // TODO
							.build(app)?,
					)
					.build()?,
			);

		#[cfg(debug_assertions)]
		let view_menu = view_menu.separator().item(
			&MenuItemBuilder::with_id(MenuEvent::ToggleDeveloperTools, "Toggle Developer Tools")
				.accelerator("CmdOrCtrl+Shift+Alt+I")
				.build(app)?,
		);

		let view_menu = view_menu.build()?;

		let window_menu = SubmenuBuilder::new(app, "Window")
			.minimize()
			// Disabling this fixes the new "Duplicate current tab" shortcut on macOS clients
			// ...and at the time I'm committing this we don't support multi-window so... ¯\_(ツ)_/¯
			// .item(
			// 	&MenuItemBuilder::with_id(MenuEvent::NewWindow, "New Window")
			// 		.accelerator("CmdOrCtrl+Shift+N")
			// 		.build(app)?,
			// )
			.fullscreen()
			.item(
				&MenuItemBuilder::with_id(MenuEvent::ReloadWebview, "Reload Webview")
					.accelerator("CmdOrCtrl+Shift+R")
					.build(app)?,
			)
			.build()?;

		let menu = MenuBuilder::new(app)
			.item(&app_menu)
			// .item(&file_menu)
			.item(&edit_menu)
			.item(&view_menu)
			.item(&window_menu)
			.build()?;

		for event in LIBRARY_LOCKED_MENU_IDS {
			set_enabled(&menu, *event, false);
		}

		Ok(menu)
	}
}

pub fn handle_menu_event(event: MenuEvent, app: &AppHandle) {
	let webview = app
		.get_webview_window("main")
		.expect("unable to find window");

	match event {
		// TODO: Use Tauri Specta with frontend instead of this
		MenuEvent::NewLibrary => webview.emit("keybind", "new_library").unwrap(),
		MenuEvent::NewFile => webview.emit("keybind", "new_file").unwrap(),
		MenuEvent::NewDirectory => webview.emit("keybind", "new_directory").unwrap(),
		MenuEvent::AddLocation => webview.emit("keybind", "add_location").unwrap(),
		MenuEvent::OpenOverview => webview.emit("keybind", "open_overview").unwrap(),
		MenuEvent::OpenSearch => webview.emit("keybind", "open_search".to_string()).unwrap(),
		MenuEvent::OpenSettings => webview.emit("keybind", "open_settings").unwrap(),
		MenuEvent::ReloadExplorer => webview.emit("keybind", "reload_explorer").unwrap(),
		MenuEvent::SetLayoutGrid => webview.emit("keybind", "set_layout_grid").unwrap(),
		MenuEvent::SetLayoutList => webview.emit("keybind", "set_layout_list").unwrap(),
		MenuEvent::SetLayoutMedia => webview.emit("keybind", "set_layout_media").unwrap(),
		MenuEvent::Copy => webview.emit("keybind", "copy").unwrap(),
		MenuEvent::Cut => webview.emit("keybind", "cut").unwrap(),
		MenuEvent::Paste => webview.emit("keybind", "paste").unwrap(),
		MenuEvent::Duplicate => webview.emit("keybind", "duplicate").unwrap(),
		MenuEvent::SelectAll => webview.emit("keybind", "select_all").unwrap(),
		MenuEvent::ToggleDeveloperTools =>
		{
			#[cfg(feature = "devtools")]
			if webview.is_devtools_open() {
				webview.close_devtools();
			} else {
				webview.open_devtools();
			}
		}
		MenuEvent::NewWindow => {
			// TODO: Implement this
		}
		MenuEvent::ReloadWebview => {
			webview
				.with_webview(crate::reload_webview_inner)
				.expect("Error while reloading webview");
		}
	}
}

// Enable/disable all items in `LIBRARY_LOCKED_MENU_IDS`
pub fn refresh_menu_bar(app: &AppHandle, enabled: bool) {
	let menu = app
		.get_window("main")
		.expect("unable to find window")
		.menu()
		.expect("unable to get menu for current window");

	for event in LIBRARY_LOCKED_MENU_IDS {
		set_enabled(&menu, *event, enabled);
	}
}

pub fn set_enabled(menu: &Menu<Wry>, event: MenuEvent, enabled: bool) {
	let result = match menu.get(event.as_ref()) {
		Some(MenuItemKind::MenuItem(i)) => i.set_enabled(enabled),
		Some(MenuItemKind::Submenu(i)) => i.set_enabled(enabled),
		Some(MenuItemKind::Predefined(_)) => return,
		Some(MenuItemKind::Check(i)) => i.set_enabled(enabled),
		Some(MenuItemKind::Icon(i)) => i.set_enabled(enabled),
		None => {
			error!("Unable to get menu item: {event:?}");
			return;
		}
	};

	if let Err(e) = result {
		error!("Error setting menu item state: {e:#?}");
	}
}
