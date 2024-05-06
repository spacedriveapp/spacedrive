use sd_core::Node;
use sd_prisma::prisma::{file_path, location};

use std::{
	collections::{BTreeSet, HashMap, HashSet},
	hash::{Hash, Hasher},
	path::PathBuf,
	sync::Arc,
};

use futures::future::join_all;
use serde::Serialize;
use specta::Type;
use tauri::async_runtime::spawn_blocking;
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
	let res = if let Some(library) = node.libraries.get_library(&library).await {
		library.get_file_paths(ids).await.map_or_else(
			|e| vec![OpenFilePathResult::Internal(e.to_string())],
			|paths| {
				paths
					.into_iter()
					.map(|(id, maybe_path)| {
						if let Some(path) = maybe_path {
							let open_result = {
								#[cfg(target_os = "linux")]
								{
									sd_desktop_linux::open_file_path(path)
								}

								#[cfg(not(target_os = "linux"))]
								{
									opener::open(path)
								}
							};

							open_result
								.map(|()| OpenFilePathResult::AllGood(id))
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

#[derive(Serialize, Type)]
#[serde(tag = "t", content = "c")]
pub enum EphemeralFileOpenResult {
	Ok(PathBuf),
	Err(String),
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_ephemeral_files(paths: Vec<PathBuf>) -> Result<Vec<EphemeralFileOpenResult>, ()> {
	Ok(paths
		.into_iter()
		.map(|path| {
			if let Err(e) = {
				#[cfg(target_os = "linux")]
				{
					sd_desktop_linux::open_file_path(&path)
				}

				#[cfg(not(target_os = "linux"))]
				{
					opener::open(&path)
				}
			} {
				error!("Failed to open file: {e:#?}");
				EphemeralFileOpenResult::Err(e.to_string())
			} else {
				EphemeralFileOpenResult::Ok(path)
			}
		})
		.collect())
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

#[cfg(target_os = "macos")]
async fn get_file_path_open_apps_set(path: PathBuf) -> Option<HashSet<OpenWithApplication>> {
	let Some(path_str) = path.to_str() else {
		error!(
			"File path contains non-UTF8 characters: '{}'",
			path.display()
		);
		return None;
	};

	let res = unsafe { sd_desktop_macos::get_open_with_applications(&path_str.into()) }
		.as_slice()
		.iter()
		.map(|app| OpenWithApplication {
			url: app.url.to_string(),
			name: app.name.to_string(),
		})
		.collect::<HashSet<_>>();

	Some(res)
}

#[cfg(target_os = "linux")]
async fn get_file_path_open_apps_set(path: PathBuf) -> Option<HashSet<OpenWithApplication>> {
	Some(
		sd_desktop_linux::list_apps_associated_with_ext(&path)
			.await
			.into_iter()
			.map(|app| OpenWithApplication {
				url: app.id,
				name: app.name,
			})
			.collect::<HashSet<_>>(),
	)
}

#[cfg(target_os = "windows")]
async fn get_file_path_open_apps_set(path: PathBuf) -> Option<HashSet<OpenWithApplication>> {
	let Some(ext) = path.extension() else {
		error!("Failed to extract file extension for '{}'", path.display());
		return None;
	};

	sd_desktop_windows::list_apps_associated_with_ext(ext)
		.map_err(|e| {
			error!("{e:#?}");
		})
		.map(|handlers| {
			handlers
				.iter()
				.filter_map(|handler| {
					let (Ok(name), Ok(url)) = (
						unsafe { handler.GetUIName() }
							.map_err(|e| {
								error!("Error on '{}': {e:#?}", path.display());
							})
							.and_then(|name| {
								unsafe { name.to_string() }.map_err(|e| {
									error!("Error on '{}': {e:#?}", path.display());
								})
							}),
						unsafe { handler.GetName() }
							.map_err(|e| {
								error!("Error on '{}': {e:#?}", path.display());
							})
							.and_then(|name| {
								unsafe { name.to_string() }.map_err(|e| {
									error!("Error on '{}': {e:#?}", path.display());
								})
							}),
					) else {
						error!("Failed to get handler info for '{}'", path.display());
						return None;
					};

					Some(OpenWithApplication { name, url })
				})
				.collect::<HashSet<_>>()
		})
		.ok()
}

async fn aggregate_open_with_apps(
	paths: impl Iterator<Item = PathBuf>,
) -> Result<Vec<OpenWithApplication>, ()> {
	Ok(join_all(paths.map(get_file_path_open_apps_set))
		.await
		.into_iter()
		.flatten()
		.reduce(|intersection, set| intersection.intersection(&set).cloned().collect())
		.map(|set| set.into_iter().collect())
		.unwrap_or(vec![]))
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_file_path_open_with_apps(
	library: uuid::Uuid,
	ids: Vec<i32>,
	node: NodeState<'_>,
) -> Result<Vec<OpenWithApplication>, ()> {
	let Some(library) = node.libraries.get_library(&library).await else {
		return Ok(vec![]);
	};

	let Ok(paths) = library.get_file_paths(ids).await.map_err(|e| {
		error!("{e:#?}");
	}) else {
		return Ok(vec![]);
	};

	aggregate_open_with_apps(paths.into_values().filter_map(|maybe_path| {
		if maybe_path.is_none() {
			error!("File not found in database");
		}
		maybe_path
	}))
	.await
}

#[tauri::command(async)]
#[specta::specta]
pub async fn get_ephemeral_files_open_with_apps(
	paths: Vec<PathBuf>,
) -> Result<Vec<OpenWithApplication>, ()> {
	aggregate_open_with_apps(paths.into_iter()).await
}

type FileIdAndUrl = (i32, String);

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path_with(
	library: uuid::Uuid,
	file_ids_and_urls: Vec<FileIdAndUrl>,
	node: NodeState<'_>,
) -> Result<(), ()> {
	let Some(library) = node.libraries.get_library(&library).await else {
		return Err(());
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
						#[cfg(any(target_os = "windows", target_os = "linux"))]
						path.as_ref(),
						#[cfg(target_os = "macos")]
						path.as_ref()
							.and_then(|path| path.to_str().map(str::to_string)),
						url_by_id.get(id),
					) else {
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

					#[cfg(target_os = "windows")]
					return sd_desktop_windows::open_file_path_with(path, url).map_err(|e| {
						error!("{e:#?}");
					});

					#[cfg(not(any(
						target_os = "windows",
						target_os = "linux",
						target_os = "macos"
					)))]
					Err(())
				})
				.collect::<Result<Vec<_>, _>>()
				.map(|_| ())
		})
}

type PathAndUrl = (PathBuf, String);

#[tauri::command(async)]
#[specta::specta]
pub async fn open_ephemeral_file_with(paths_and_urls: Vec<PathAndUrl>) -> Result<(), ()> {
	join_all(
		paths_and_urls
			.into_iter()
			.collect::<HashMap<_, _>>() // Just to avoid duplicates
			.into_iter()
			.map(|(path, url)| async move {
				#[cfg(target_os = "macos")]
				if let Some(path) = path.to_str().map(str::to_string) {
					if let Err(e) = spawn_blocking(move || {
						sd_desktop_macos::open_file_paths_with(&[path], &url);
					})
					.await
					{
						error!("Error joining spawned task for opening files with: {e:#?}");
					}
				} else {
					error!(
						"File path contains non-UTF8 characters: '{}'",
						path.display()
					);
				};

				#[cfg(target_os = "linux")]
				match spawn_blocking(move || sd_desktop_linux::open_files_path_with(&[path], &url))
					.await
				{
					Ok(Ok(())) => (),
					Ok(Err(e)) => error!("Error opening file with: {e:#?}"),
					Err(e) => error!("Error joining spawned task for opening files with: {e:#?}"),
				}

				#[cfg(windows)]
				match spawn_blocking(move || sd_desktop_windows::open_file_path_with(path, &url))
					.await
				{
					Ok(Ok(())) => (),
					Ok(Err(e)) => error!("Error opening file with: {e:#?}"),
					Err(e) => error!("Error joining spawned task for opening files with: {e:#?}"),
				}
			}),
	)
	.await;

	Ok(())
}

fn inner_reveal_paths(paths: impl Iterator<Item = PathBuf>) {
	for path in paths {
		if let Err(e) = opener::reveal(path) {
			error!("Failed to open logs dir: {e:#?}");
		}
	}
}

#[derive(specta::Type, serde::Deserialize)]
pub enum RevealItem {
	Location { id: location::id::Type },
	FilePath { id: file_path::id::Type },
	Ephemeral { path: PathBuf },
}

#[tauri::command(async)]
#[specta::specta]
pub async fn reveal_items(
	library: uuid::Uuid,
	items: Vec<RevealItem>,
	node: NodeState<'_>,
) -> Result<(), ()> {
	let Some(library) = node.libraries.get_library(&library).await else {
		return Err(());
	};

	let mut paths_to_open = BTreeSet::new();

	let (paths, locations): (Vec<_>, Vec<_>) =
		items
			.into_iter()
			.fold((vec![], vec![]), |(mut paths, mut locations), item| {
				match item {
					RevealItem::FilePath { id } => paths.push(id),
					RevealItem::Location { id } => locations.push(id),
					RevealItem::Ephemeral { path } => {
						paths_to_open.insert(path);
					}
				}

				(paths, locations)
			});

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
					location::instance_id::equals(Some(library.config().await.instance_id)),
					location::id::in_vec(locations),
				])
				.select(location::select!({ path }))
				.exec()
				.await
				.unwrap_or_default()
				.into_iter()
				.filter_map(|location| location.path.map(Into::into)),
		);
	}

	if let Err(e) = spawn_blocking(|| inner_reveal_paths(paths_to_open.into_iter())).await {
		error!("Error joining reveal paths thread: {e:#?}");
	}

	Ok(())
}
