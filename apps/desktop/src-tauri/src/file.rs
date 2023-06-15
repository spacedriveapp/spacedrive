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
	let Some(library) = node.library_manager.get_library(library).await
		else {
			return Ok(vec![]);
		};

	let Ok(paths) = library
		.get_file_paths(ids).await
		.map_err(|e| {error!("{e:#?}");})
		else {
			return Ok(vec![]);
		};

	#[cfg(target_os = "macos")]
	return Ok(paths
		.into_iter()
		.flat_map(|(id, path)| {
			let Some(path) = path
				else {
					error!("File not found in database");
					return vec![];
				};

			unsafe { sd_desktop_macos::get_open_with_applications(&path.to_str().unwrap().into()) }
				.as_slice()
				.iter()
				.map(|app| OpenWithApplication::File {
					id,
					name: app.name.to_string(),
					url: app.url.to_string(),
				})
				.collect::<Vec<_>>()
		})
		.collect());

	#[cfg(target_os = "linux")]
	{
		use sd_desktop_linux::{DesktopEntry, HandlerType, SystemApps};

		// TODO: cache this, and only update when the underlying XDG desktop apps changes
		let Ok(system_apps) = SystemApps::populate()
			.map_err(|e| { error!("{e:#?}"); })
			else {
				return Ok(vec![]);
			};

		return Ok(paths
			.into_iter()
			.flat_map(|(id, path)| {
				let Some(path) = path
					else {
						error!("File not found in database");
						return vec![];
					};

				let Some(name) = path.file_name()
					.and_then(|name| name.to_str())
					.map(|name| name.to_string())
					else {
						error!("Failed to extract file name");
						return vec![];
					};

				system_apps
					.get_handlers(HandlerType::Ext(name))
					.iter()
					.map(|handler| {
						handler
							.get_path()
							.map_err(|e| {
								error!("{e:#?}");
							})
							.and_then(|path| {
								DesktopEntry::try_from(&path)
									// TODO: Ignore desktop entries that have commands that don't exist/aren't available in path
									.map(|entry| OpenWithApplication::File {
										id,
										name: entry.name,
										url: path,
									})
									.map_err(|e| {
										error!("{e:#?}");
									})
							})
					})
					.collect::<Result<Vec<_>, _>>()
					.unwrap_or(vec![])
			})
			.collect());
	}

	#[cfg(windows)]
	{
		use sd_desktop_windows::list_apps_associated_with_ext;

		return Ok(paths
			.into_iter()
			.flat_map(|(id, path)| {
				let Some(path) = path
					else {
						error!("File not found in database");
						return vec![];
					};

				let Some(ext) = path.extension()
					else {
						error!("Failed to extract file extension");
						return vec![];
					};

				list_apps_associated_with_ext(ext)
					.map_err(|e| {
						error!("{e:#?}");
					})
					.map(|handlers| {
						handlers
							.iter()
							.filter_map(|handler| {
								let (Ok(name), Ok(url)) = (
								unsafe { handler.GetUIName() }.map_err(|e| { error!("{e:#?}");})
									.and_then(|name| unsafe { name.to_string() }
									.map_err(|e| { error!("{e:#?}");})),
								unsafe { handler.GetName() }.map_err(|e| { error!("{e:#?}");})
									.and_then(|name| unsafe { name.to_string() }
									.map_err(|e| { error!("{e:#?}");})),
							) else {
								error!("Failed to get handler info");
								return None
							};

								Some(OpenWithApplication::File { id, name, url })
							})
							.collect::<Vec<_>>()
					})
					.unwrap_or(vec![])
			})
			.collect());
	}

	#[allow(unreachable_code)]
	Ok(vec![])
}

type FileIdAndUrl = (i32, String);

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	file_ids_and_urls: Vec<FileIdAndUrl>,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<(), ()> {
	let Some(library) = node.library_manager.get_library(library).await
		else {
			return Err(())
		};

	let url_by_id = file_ids_and_urls.into_iter().collect::<HashMap<_, _>>();
	let ids = url_by_id.keys().copied().collect::<Vec<_>>();

	library
		.get_file_paths(ids)
		.await
		.map_err(|e| {
			error!("{e:#?}");
		})
		.and_then(|paths| {
			paths
				.iter()
				.map(|(id, path)| {
					let (Some(path), Some(url)) = (
						#[cfg(windows)]
						path.as_ref(),
						#[cfg(not(windows))]
						path.as_ref().and_then(|path| path.to_str()),
						url_by_id.get(id)
					)
						else {
							error!("File not found in database");
							return Err(());
						};

					#[cfg(target_os = "macos")]
					return {
						unsafe {
							sd_desktop_macos::open_file_path_with(
								&path.into(),
								&url.as_str().into(),
							)
						};
						Ok(())
					};

					#[cfg(target_os = "linux")]
					{
						return sd_desktop_linux::Handler::assume_valid(url.into())
							.open(&[path])
							.map_err(|e| {
								error!("{e:#?}");
							});
					};

					#[cfg(windows)]
					return sd_desktop_windows::open_file_path_with(path, url).map_err(|e| {
						error!("{e:#?}");
					});

					#[allow(unreachable_code)]
					Err(())
				})
				.collect::<Result<Vec<_>, _>>()
				.map(|_| ())
		})
}
