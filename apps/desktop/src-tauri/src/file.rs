use std::{collections::HashMap, sync::Arc};

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
#[allow(dead_code)]
pub enum OpenWithApplication {
	File {
		id: i32,
		name: String,
		#[cfg(target_os = "linux")]
		url: std::path::PathBuf,
		#[cfg(not(target_os = "linux"))]
		url: String,
	},
	Error(i32, String),
}

#[tauri::command(async)]
#[specta::specta]
#[allow(unused_variables)]
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

	#[cfg(target_os = "linux")]
	{
		use sd_desktop_linux::{DesktopEntry, HandlerType, SystemApps};

		// TODO: cache this, and only update when the underlying XDG desktop apps changes
		let system_apps = SystemApps::populate().map_err(|_| ())?;

		return Ok(paths
			.into_iter()
			.flat_map(|(id, path)| {
				if let Some(path) = path {
					let Some(name) = path.file_name()
						.and_then(|name| name.to_str())
						.map(|name| name.to_string())
					else {
						return vec![OpenWithApplication::Error(
							id,
							"Failed to extract file name".into(),
						)]
					};

					system_apps
						.get_handlers(HandlerType::Ext(name))
						.iter()
						.map(|handler| {
							handler
								.get_path()
								.map(|path| {
									DesktopEntry::try_from(&path)
										.map(|entry| OpenWithApplication::File {
											id,
											name: entry.name,
											url: path,
										})
										.unwrap_or_else(|e| {
											error!("{e:#?}");
											OpenWithApplication::Error(
												id,
												"Failed to parse desktop entry".into(),
											)
										})
								})
								.unwrap_or_else(|e| {
									error!("{e:#?}");
									OpenWithApplication::Error(
										id,
										"Failed to get path from desktop entry".into(),
									)
								})
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

	#[cfg(windows)]
	{
		use sd_desktop_windows::{list_apps_associated_with_ext, Error, Result};

		return Ok(list_apps_associated_with_ext(path.extension().ok_or(())?)
			.map_err(|_| ())?
			.iter()
			.filter_map(|handler| {
				if let (Ok(name), Ok(url)) = (
					unsafe { handler.GetUIName() }.and_then(|name| -> Result<_> {
						unsafe { name.to_string() }.map_err(|_| Error::OK)
					}),
					unsafe { handler.GetName() }.and_then(|name| -> Result<_> {
						unsafe { name.to_string() }.map_err(|_| Error::OK)
					}),
				) {
					Some(OpenWithApplication { name, url })
				} else {
					None
				}
			})
			.collect::<Vec<OpenWithApplication>>());
	}

	#[allow(unreachable_code)]
	Err(())
}

type FileIdAndUrl = (i32, String);

#[tauri::command(async)]
#[specta::specta]
#[allow(unused_variables)]
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

	#[cfg(target_os = "linux")]
	{
		library
			.get_file_paths(ids)
			.await
			.map(|paths| {
				paths.iter().for_each(|(id, path)| {
					if let Some(path) = path.as_ref().and_then(|path| path.to_str()) {
						if let Err(e) = sd_desktop_linux::Handler::assume_valid(
							url_by_id
								.get(id)
								.expect("we just created this hashmap")
								.as_str()
								.into(),
						)
						.open(&[path])
						{
							error!("{e:#?}");
						}
					}
				})
			})
			.map_err(|e| {
				error!("{e:#?}");
			})
	}

	#[cfg(windows)]
	{
		sd_desktop_windows::open_file_path_with(&path, url).map_err(|_| ())?;
	}

	Ok(())
}
