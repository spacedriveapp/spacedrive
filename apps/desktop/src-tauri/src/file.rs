use std::{
	collections::{BTreeSet, HashMap, HashSet},
	hash::{Hash, Hasher},
	sync::Arc,
};

use sd_core::{
	prisma::{file_path, location},
	Node,
};
use serde::Serialize;
use specta::Type;
use tracing::error;

type NodeState<'a> = tauri::State<'a, Arc<Node>>;

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
pub async fn open_file_paths(
	library: uuid::Uuid,
	ids: Vec<i32>,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<Vec<OpenFilePathResult>, ()> {
	let res = if let Some(library) = node.library_manager.get_library(&library).await {
		library.get_file_paths(ids).await.map_or_else(
			|e| vec![OpenFilePathResult::Internal(e.to_string())],
			|paths| {
				paths
					.into_iter()
					.map(|(id, maybe_path)| {
						if let Some(path) = maybe_path {
							#[cfg(target_os = "linux")]
							let open_result = sd_desktop_linux::open_file_path(&path);

							#[cfg(not(target_os = "linux"))]
							let open_result = opener::open(path);

							open_result
								.map(|_| OpenFilePathResult::AllGood(id))
								.unwrap_or_else(|err| {
									error!("Failed to open logs dir: {err}");
									OpenFilePathResult::OpenError(id, err.to_string())
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

#[derive(Serialize, Type, Debug, Clone)]
pub struct OpenWithApplication {
	url: String,
	name: String,
}

impl Hash for OpenWithApplication {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.url.hash(state);
	}
}

impl PartialEq for OpenWithApplication {
	fn eq(&self, other: &Self) -> bool {
		self.url == other.url
	}
}

impl Eq for OpenWithApplication {}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_file_path_open_with_apps(
	library: uuid::Uuid,
	ids: Vec<i32>,
	node: NodeState<'_>,
) -> Result<Vec<OpenWithApplication>, ()> {
	let Some(library) = node.library_manager.get_library(&library).await
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
	return {
		Ok(paths
			.into_values()
			.flat_map(|path| {
				let Some(path) = path.and_then(|path| path.into_os_string().into_string().ok())
					else {
						error!("File not found in database");
						return None;
					};

				Some(
					unsafe { sd_desktop_macos::get_open_with_applications(&path.as_str().into()) }
						.as_slice()
						.iter()
						.map(|app| OpenWithApplication {
							url: app.url.to_string(),
							name: app.name.to_string(),
						})
						.collect::<HashSet<_>>(),
				)
			})
			.reduce(|intersection, set| intersection.intersection(&set).cloned().collect())
			.map(|set| set.into_iter().collect())
			.unwrap_or(vec![]))
	};

	#[cfg(target_os = "linux")]
	{
		use futures::future;
		use sd_desktop_linux::list_apps_associated_with_ext;

		let apps = future::join_all(paths.into_values().map(|path| async {
			let Some(path) = path
					else {
						error!("File not found in database");
						return None;
					};

			Some(
				list_apps_associated_with_ext(&path)
					.await
					.into_iter()
					.map(|app| OpenWithApplication {
						url: app.id,
						name: app.name,
					})
					.collect::<HashSet<_>>(),
			)
		}))
		.await;

		return Ok(apps
			.into_iter()
			.flatten()
			.reduce(|intersection, set| intersection.intersection(&set).cloned().collect())
			.map(|set| set.into_iter().collect())
			.unwrap_or(vec![]));
	}

	#[cfg(windows)]
	return Ok(paths
		.into_values()
		.filter_map(|path| {
			let Some(path) = path
				else {
					error!("File not found in database");
					return None;
				};

			let Some(ext) = path.extension()
				else {
					error!("Failed to extract file extension");
					return None;
				};

			sd_desktop_windows::list_apps_associated_with_ext(ext)
				.map_err(|e| {
					error!("{e:#?}");
				})
				.ok()
		})
		.map(|handler| {
			handler
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

					Some(OpenWithApplication { name, url })
				})
				.collect::<HashSet<_>>()
		})
		.reduce(|intersection, set| intersection.intersection(&set).cloned().collect())
		.map(|set| set.into_iter().collect())
		.unwrap_or(vec![]));

	#[allow(unreachable_code)]
	Ok(vec![])
}

type FileIdAndUrl = (i32, String);

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	file_ids_and_urls: Vec<FileIdAndUrl>,
	node: NodeState<'_>,
) -> Result<(), ()> {
	let Some(library) = node.library_manager.get_library(&library).await
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
						sd_desktop_macos::open_file_paths_with(&[path], url);
						Ok(())
					};

					#[cfg(target_os = "linux")]
					return sd_desktop_linux::open_files_path_with(&[path], url).map_err(|e| {
						error!("{e:#?}");
					});

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

#[derive(specta::Type, serde::Deserialize)]
pub enum RevealItem {
	Location { id: location::id::Type },
	FilePath { id: file_path::id::Type },
}

#[tauri::command(async)]
#[specta::specta]
pub async fn reveal_items(
	library: uuid::Uuid,
	items: Vec<RevealItem>,
	node: NodeState<'_>,
) -> Result<(), ()> {
	let Some(library) = node.library_manager.get_library(&library).await
		else {
			return Err(())
		};

	let (paths, locations): (Vec<_>, Vec<_>) =
		items
			.into_iter()
			.fold((vec![], vec![]), |(mut paths, mut locations), item| {
				match item {
					RevealItem::FilePath { id } => paths.push(id),
					RevealItem::Location { id } => locations.push(id),
				}

				(paths, locations)
			});

	let mut paths_to_open = BTreeSet::new();

	if !paths.is_empty() {
		paths_to_open.extend(
			library
				.get_file_paths(paths)
				.await
				.unwrap_or_default()
				.into_values()
				.flatten(),
		);
	}

	if !locations.is_empty() {
		paths_to_open.extend(
			library
				.db
				.location()
				.find_many(vec![
					// TODO(N): This will fall apart with removable media and is making an invalid assumption that the `Node` is fixed for an `Instance`.
					location::instance_id::equals(Some(library.config.instance_id)),
					location::id::in_vec(locations),
				])
				.select(location::select!({ path }))
				.exec()
				.await
				.unwrap_or_default()
				.into_iter()
				.flat_map(|location| location.path.map(Into::into)),
		);
	}

	for path in paths_to_open {
		#[cfg(target_os = "linux")]
		if sd_desktop_linux::is_appimage() {
			// This is a workaround for the app, when package inside an AppImage, crashing when using opener::reveal.
			sd_desktop_linux::open_file_path(
				&(if path.is_file() {
					path.parent().unwrap_or(&path)
				} else {
					&path
				}),
			)
			.map_err(|err| {
				error!("Failed to open logs dir: {err}");
			})
			.ok()
		} else {
			opener::reveal(path)
				.map_err(|err| {
					error!("Failed to open logs dir: {err}");
				})
				.ok()
		};

		#[cfg(not(target_os = "linux"))]
		opener::reveal(path)
			.map_err(|err| {
				error!("Failed to open logs dir: {err}");
			})
			.ok();
	}

	Ok(())
}
