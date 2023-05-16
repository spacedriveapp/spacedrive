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

#[derive(Type, serde::Serialize)]
pub struct OpenWithApplication {
	name: String,
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
	.into_iter()
	.map(|app| OpenWithApplication {
		name: app.name.to_string(),
		url: app.url.to_string(),
	})
	.collect());

	#[cfg(not(target_os = "macos"))]
	Err(())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	id: i32,
	with_url: String,
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
			&path.to_str().unwrap().into(),
			&with_url.as_str().into(),
		)
	};

	Ok(())
}
