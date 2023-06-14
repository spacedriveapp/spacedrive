use std::sync::Arc;

use sd_core::Node;
use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
#[serde(tag = "t", content = "c")]
pub enum OpenFilePathResult {
	NoLibrary,
	NoFile,
	OpenError(String),
	AllGood,
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path(
	library: uuid::Uuid,
	id: i32,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<OpenFilePathResult, ()> {
	let res = if let Some(library) = node.library_manager.get_library(library).await {
		let Ok(Some(path)) = library
			.get_file_path(id)
			.await
            else {
                return Ok(OpenFilePathResult::NoFile)
            };

		opener::open(path)
			.map(|_| OpenFilePathResult::AllGood)
			.unwrap_or_else(|e| OpenFilePathResult::OpenError(e.to_string()))
	} else {
		OpenFilePathResult::NoLibrary
	};

	Ok(res)
}

#[derive(Type, Debug, serde::Serialize)]
pub struct OpenWithApplication {
	name: String,
	#[cfg(target_os = "linux")]
	url: std::path::PathBuf,
	#[cfg(not(target_os = "linux"))]
	url: String,
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_file_path_open_with_apps(
	library: uuid::Uuid,
	id: i32,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<OpenWithApplication>, ()> {
	let Some(library) = node.library_manager.get_library(library).await else {
        return Err(())
    };

	let Ok(Some(path)) = library
        .get_file_path(id)
        .await
        else {
            return Err(())
        };

	#[cfg(target_os = "macos")]
	return Ok(unsafe {
		sd_desktop_macos::get_open_with_applications(&path.to_str().unwrap().into())
	}
	.as_slice()
	.iter()
	.map(|app| OpenWithApplication {
		name: app.name.to_string(),
		url: app.url.to_string(),
	})
	.collect());

	#[cfg(target_os = "linux")]
	{
		use sd_desktop_linux::{DesktopEntry, HandlerType, SystemApps};

		// TODO: cache this, and only update when the underlying XDG desktop apps changes
		let system_apps = SystemApps::populate().map_err(|_| ())?;

		let handlers = system_apps.get_handlers(HandlerType::Ext(
			path.file_name()
				.and_then(|name| name.to_str())
				.map(|name| name.to_string())
				.ok_or(
					// io::Error::new(
					// 	io::ErrorKind::Other,
					// 	"Missing file name from path",
					// )
					(),
				)?,
		));

		let data = handlers
			.iter()
			.map(|handler| {
				let path = handler.get_path().map_err(|_| ())?;
				let entry = DesktopEntry::try_from(path.clone()).map_err(|_| ())?;
				Ok(OpenWithApplication {
					name: entry.name,
					url: path,
				})
			})
			.collect::<Result<Vec<OpenWithApplication>, _>>()?;

		return Ok(data);
	}

	#[allow(unreachable_code)]
	Err(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	id: i32,
	url: String,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<(), ()> {
	let Some(library) = node.library_manager.get_library(library).await else {
        return Err(())
    };

	let Ok(Some(path)) = library
        .get_file_path(id)
        .await
        else {
            return Err(())
        };

	#[cfg(target_os = "macos")]
	unsafe {
		sd_desktop_macos::open_file_path_with(
			&path.to_str().ok_or(())?.into(),
			&url.as_str().into(),
		)
	};

	#[cfg(target_os = "linux")]
	{
		sd_desktop_linux::Handler::assume_valid(url.into())
			.open(&[path.to_str().ok_or(())?])
			.map_err(|_| ())?;
	}

	Ok(())
}
