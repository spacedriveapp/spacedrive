use std::path::Path;

use gtk::{
	gio::{
		content_type_guess, prelude::AppInfoExt, AppInfo, AppLaunchContext, DesktopAppInfo,
		File as GioFile, ResourceError,
	},
	glib::error::Error as GlibError,
};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub struct App {
	pub id: String,
	pub name: String,
	// pub icon: Option<Vec<u8>>,
}

async fn recommended_for_type(file_path: impl AsRef<Path>) -> Vec<AppInfo> {
	let data = if let Ok(mut file) = File::open(&file_path).await {
		let mut data = [0; 1024];
		if file.read_exact(&mut data).await.is_ok() {
			Some(data)
		} else {
			None
		}
	} else {
		None
	};

	let file_path = Some(file_path);
	let (content_type, uncertain) = if let Some(data) = data {
		content_type_guess(file_path, &data)
	} else {
		content_type_guess(file_path, &[])
	};

	if uncertain {
		vec![]
	} else {
		AppInfo::recommended_for_type(content_type.as_str())
	}
}

pub async fn list_apps_associated_with_ext(file_path: impl AsRef<Path>) -> Vec<App> {
	recommended_for_type(file_path)
		.await
		.iter()
		.flat_map(|app_info| {
			app_info.id().map(|id| App {
				id: id.to_string(),
				name: app_info.name().to_string(),
				// TODO: Icon supports requires GTK, which can only run on the main thread
				// icon: app_info
				// 	.icon()
				// 	.and_then(|icon| {
				// 		IconTheme::default().and_then(|icon_theme| {
				// 			icon_theme.lookup_by_gicon(&icon, 128, IconLookupFlags::empty())
				// 		})
				// 	})
				// 	.and_then(|icon_info| icon_info.load_icon().ok())
				// 	.and_then(|pixbuf| pixbuf.save_to_bufferv("png", &[]).ok()),
			})
		})
		.collect()
}

pub fn open_files_path_with(file_paths: &[impl AsRef<Path>], id: &str) -> Result<(), GlibError> {
	if let Some(app) = DesktopAppInfo::new(id) {
		app.launch(
			&file_paths.iter().map(GioFile::for_path).collect::<Vec<_>>(),
			None::<&AppLaunchContext>,
			// TODO: Get default display requires GDK, which can only run on the main thread
			// Display::default()
			// 	.and_then(|display| display.app_launch_context())
			// 	.as_ref(),
		)
	} else {
		Err(GlibError::new(ResourceError::NotFound, "App not found"))
	}
}
