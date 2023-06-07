use std::collections::HashMap;
use std::sync::Arc;

use sd_core::Node;
use serde::Serialize;
use specta::Type;
use tracing::error;

#[derive(Serialize, Type)]
#[serde(tag = "t", content = "c")]
pub enum OpenFilePathResult {
	NoLibrary,
	NoFile(i32),
	OpenError(i32, String),
	AllGood(i32),
	Internal(String),
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path(
	library: uuid::Uuid,
	ids: Vec<i32>,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<OpenFilePathResult>, ()> {
	let res = if let Some(library) = node.library_manager.get_library(library).await {
		library.get_file_paths(ids).await.map_or_else(
			|e| vec![OpenFilePathResult::Internal(e.to_string())],
			|paths| {
				paths
					.into_iter()
					.map(|(id, maybe_path)| {
						if let Some(path) = maybe_path {
							opener::open(path)
								.map(|_| OpenFilePathResult::AllGood(id))
								.unwrap_or_else(|e| {
									OpenFilePathResult::OpenError(id, e.to_string())
								})
						} else {
							OpenFilePathResult::NoFile(id)
						}
					})
					.collect()
			},
		)
	} else {
		vec![OpenFilePathResult::NoLibrary]
	};

	Ok(res)
}

#[derive(Serialize, Type)]
#[serde(tag = "t", content = "c")]
pub enum OpenWithApplication {
	File { id: i32, name: String, url: String },
	Error(i32, String),
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_file_path_open_with_apps(
	library: uuid::Uuid,
	ids: Vec<i32>,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<OpenWithApplication>, ()> {
	let Some(library) = node.library_manager.get_library(library).await else {
        return Err(())
    };

	let Ok(paths) = library.get_file_paths(ids).await.map_err(|e| {error!("{e:#?}");})
	else {
		return Err(());
	};

	#[cfg(not(target_os = "macos"))]
	return Err(());

	#[cfg(target_os = "macos")]
	return Ok(paths
		.into_iter()
		.flat_map(|(id, path)| {
			if let Some(path) = path {
				unsafe {
					sd_desktop_macos::get_open_with_applications(&path.to_str().unwrap().into())
				}
				.as_slice()
				.iter()
				.map(|app| OpenWithApplication::File {
					id,
					name: app.name.to_string(),
					url: app.url.to_string(),
				})
				.collect::<Vec<_>>()
			} else {
				vec![OpenWithApplication::Error(
					id,
					"File not found in database".into(),
				)]
			}
		})
		.collect());
}

type FileIdAndUrl = (i32, String);

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	file_ids_and_urls: Vec<FileIdAndUrl>,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<(), ()> {
	let Some(library) = node.library_manager.get_library(library).await else {
        return Err(())
    };

	let url_by_id = file_ids_and_urls.into_iter().collect::<HashMap<_, _>>();
	let ids = url_by_id.keys().copied().collect::<Vec<_>>();

	#[cfg(target_os = "macos")]
	{
		library
			.get_file_paths(ids)
			.await
			.map(|paths| {
				paths.iter().for_each(|(id, path)| {
					if let Some(path) = path {
						unsafe {
							sd_desktop_macos::open_file_path_with(
								&path.to_str().unwrap().into(),
								&url_by_id
									.get(id)
									.expect("we just created this hashmap")
									.as_str()
									.into(),
							)
						}
					}
				})
			})
			.map_err(|e| {
				error!("{e:#?}");
			})
	}

	#[cfg(not(target_os = "macos"))]
	{
		Err(())
	}
}
