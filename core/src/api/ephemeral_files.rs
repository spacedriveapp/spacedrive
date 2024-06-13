use crate::{
	api::{
		files::{create_file, MediaData},
		utils::library,
	},
	invalidate_query,
	library::Library,
	object::{
		fs::{error::FileSystemJobsError, find_available_filename_for_duplicate},
		// media::exif_metadata_extractor::{can_extract_exif_data_for_image, extract_exif_data},
	},
};

use async_channel::Receiver;
use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_heavy_lifting::media_processor::exif_media_data;

use sd_file_ext::{
	extensions::{Extension, ImageExtension},
	kind::ObjectKind,
};
use sd_media_metadata::FFmpegMetadata;
use sd_utils::error::FileIOError;

use std::{ffi::OsStr, path::PathBuf, str::FromStr, time::Duration};

use futures_concurrency::future::{Join, TryJoin};
use regex::Regex;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tokio::{fs, io};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tracing::{error, warn};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use trash;

use super::{
	files::{create_directory, FromPattern},
	Ctx, R,
};

const UNTITLED_FOLDER_STR: &str = "Untitled Folder";
const UNTITLED_FILE_STR: &str = "Untitled";
const UNTITLED_TEXT_FILE_STR: &str = "Untitled.txt";

#[derive(Type, Deserialize)]
#[serde(rename_all = "camelCase")]
enum EphemeralFileCreateContextTypes {
	Empty,
	Text,
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("getMediaData", {
			R.query(|_, full_path: PathBuf| async move {
				let kind: Option<ObjectKind> = Extension::resolve_conflicting(&full_path, false)
					.await
					.map(Into::into);
				match kind {
					Some(ObjectKind::Image) => {
						let Some(extension) = full_path.extension().and_then(|ext| ext.to_str())
						else {
							return Ok(None);
						};

						let image_extension = ImageExtension::from_str(extension).map_err(|e| {
							error!(?e, "Failed to parse image extension;");
							rspc::Error::new(
								ErrorCode::BadRequest,
								"Invalid image extension".to_string(),
							)
						})?;

						if !exif_media_data::can_extract(image_extension) {
							return Ok(None);
						}

						let exif_data = exif_media_data::extract(full_path)
							.await
							.map_err(|e| {
								rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to extract media data".to_string(),
									e,
								)
							})?
							.map(MediaData::Exif);

						Ok(exif_data)
					}
					Some(v) if v == ObjectKind::Audio || v == ObjectKind::Video => {
						let ffmpeg_data = MediaData::FFmpeg(
							FFmpegMetadata::from_path(full_path).await.map_err(|e| {
								error!(?e, "Failed to extract ffmpeg metadata;");
								rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									e.to_string(),
									e,
								)
							})?,
						);

						Ok(Some(ffmpeg_data))
					}
					_ => Ok(None), // No media data
				}
			})
		})
		.procedure("createFolder", {
			#[derive(Type, Deserialize)]
			pub struct CreateEphemeralFolderArgs {
				pub path: PathBuf,
				pub name: Option<String>,
			}
			R.with2(library()).mutation(
				|(_, library),
				 CreateEphemeralFolderArgs { mut path, name }: CreateEphemeralFolderArgs| async move {
					path.push(name.as_deref().unwrap_or(UNTITLED_FOLDER_STR));

					create_directory(path, &library).await
				},
			)
		})
		.procedure("createFile", {
			#[derive(Type, Deserialize)]
			pub struct CreateEphemeralFileArgs {
				pub path: PathBuf,
				pub context: EphemeralFileCreateContextTypes,
				pub name: Option<String>,
			}
			R.with2(library()).mutation(
				|(_, library),
				 CreateEphemeralFileArgs {
				     mut path,
				     name,
				     context,
				 }: CreateEphemeralFileArgs| async move {
					match context {
						EphemeralFileCreateContextTypes::Empty => {
							path.push(name.as_deref().unwrap_or(UNTITLED_FILE_STR));
						}
						EphemeralFileCreateContextTypes::Text => {
							path.push(name.as_deref().unwrap_or(UNTITLED_TEXT_FILE_STR));
						}
					}

					create_file(path, &library).await
				},
			)
		})
		.procedure("deleteFiles", {
			R.with2(library())
				.mutation(|(_, library), paths: Vec<PathBuf>| async move {
					paths
						.into_iter()
						.map(|path| async move {
							match fs::metadata(&path).await {
								Ok(metadata) => if metadata.is_dir() {
									fs::remove_dir_all(&path).await
								} else {
									fs::remove_file(&path).await
								}
								.map_err(|e| FileIOError::from((path, e, "Failed to delete file"))),
								Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
								Err(e) => Err(FileIOError::from((
									path,
									e,
									"Failed to get file metadata for deletion",
								))),
							}
						})
						.collect::<Vec<_>>()
						.try_join()
						.await?;

					invalidate_query!(library, "search.ephemeralPaths");

					Ok(())
				})
		})
		.procedure("moveToTrash", {
			R.with2(library())
				.mutation(|(_, library), paths: Vec<PathBuf>| async move {
					if cfg!(target_os = "ios") || cfg!(target_os = "android") {
						return Err(rspc::Error::new(
							ErrorCode::MethodNotSupported,
							"Moving to trash is not supported on this platform".to_string(),
						));
					}

					paths
						.into_iter()
						.map(|path| async move {
							match fs::metadata(&path).await {
								Ok(_) => {
									#[cfg(not(any(target_os = "ios", target_os = "android")))]
									trash::delete(&path).map_err(|e| {
										FileIOError::from((
											path,
											match e {
												#[cfg(all(unix, not(target_os = "macos")))]
												trash::Error::FileSystem { path: _, source: e } => e,
												_ => io::Error::other(e),
											},
											"Failed to delete file",
										))
									})?;

									Ok::<_, rspc::Error>(())
								}
								Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
								Err(e) => Err(FileIOError::from((
									path,
									e,
									"Failed to get file metadata for deletion",
								))
								.into()),
							}
						})
						.collect::<Vec<_>>()
						.try_join()
						.await?;

					invalidate_query!(library, "search.ephemeralPaths");

					Ok(())
				})
		})
		.procedure("copyFiles", {
			R.with2(library())
				.mutation(|(_, library), args: EphemeralFileSystemOps| async move {
					args.copy(&library).await
				})
		})
		.procedure("cutFiles", {
			R.with2(library())
				.mutation(|(_, library), args: EphemeralFileSystemOps| async move {
					args.cut(&library).await
				})
		})
		.procedure("renameFile", {
			#[derive(Type, Deserialize)]
			pub struct EphemeralRenameOne {
				pub from_path: PathBuf,
				pub to: String,
			}

			#[derive(Type, Deserialize)]
			pub struct EphemeralRenameMany {
				pub from_pattern: FromPattern,
				pub to_pattern: String,
				pub from_paths: Vec<PathBuf>,
			}

			#[derive(Type, Deserialize)]
			pub enum EphemeralRenameKind {
				One(EphemeralRenameOne),
				Many(EphemeralRenameMany),
			}

			#[derive(Type, Deserialize)]
			pub struct EphemeralRenameFileArgs {
				pub kind: EphemeralRenameKind,
			}

			impl EphemeralRenameFileArgs {
				pub async fn rename_one(
					EphemeralRenameOne { from_path, to }: EphemeralRenameOne,
				) -> Result<(), rspc::Error> {
					let Some(old_name) = from_path.file_name() else {
						return Err(rspc::Error::new(
							ErrorCode::BadRequest,
							"Missing file name on file to be renamed".to_string(),
						));
					};

					if old_name == OsStr::new(&to) {
						return Ok(());
					}

					let (new_file_name, new_extension) =
						IsolatedFilePathData::separate_name_and_extension_from_str(&to).map_err(
							|e| rspc::Error::with_cause(ErrorCode::BadRequest, e.to_string(), e),
						)?;

					if !IsolatedFilePathData::accept_file_name(new_file_name) {
						return Err(rspc::Error::new(
							ErrorCode::BadRequest,
							"Invalid file name".to_string(),
						));
					}

					let Some(parent) = from_path.parent() else {
						return Err(rspc::Error::new(
							ErrorCode::BadRequest,
							"Missing parent path on file to be renamed".to_string(),
						));
					};

					let new_file_full_path = parent.join(if !new_extension.is_empty() {
						&to
					} else {
						new_file_name
					});

					match fs::metadata(&new_file_full_path).await {
						Ok(_) => Err(rspc::Error::new(
							ErrorCode::Conflict,
							"Renaming would overwrite a file".to_string(),
						)),

						Err(e) => {
							if e.kind() != std::io::ErrorKind::NotFound {
								return Err(rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to check if file exists".to_string(),
									e,
								));
							}

							fs::rename(&from_path, new_file_full_path)
								.await
								.map_err(|e| {
									FileIOError::from((from_path, e, "Failed to rename file"))
										.into()
								})
						}
					}
				}

				pub async fn rename_many(
					EphemeralRenameMany {
						ref from_pattern,
						ref to_pattern,
						from_paths,
					}: EphemeralRenameMany,
				) -> Result<(), rspc::Error> {
					let from_regex = &Regex::new(&from_pattern.pattern).map_err(|e| {
						rspc::Error::with_cause(
							rspc::ErrorCode::BadRequest,
							"Invalid `from` regex pattern".to_string(),
							e,
						)
					})?;

					from_paths
						.into_iter()
						.map(|old_path| async move {
							let Some(old_name) = old_path.file_name() else {
								return Err(rspc::Error::new(
									ErrorCode::BadRequest,
									"Missing file name on file to be renamed".to_string(),
								));
							};

							let Some(old_name_str) = old_name.to_str() else {
								return Err(rspc::Error::new(
									ErrorCode::BadRequest,
									"File with non UTF-8 name".to_string(),
								));
							};

							let replaced_full_name = if from_pattern.replace_all {
								from_regex.replace_all(old_name_str, to_pattern)
							} else {
								from_regex.replace(old_name_str, to_pattern)
							};

							if !IsolatedFilePathData::accept_file_name(replaced_full_name.as_ref())
							{
								return Err(rspc::Error::new(
									ErrorCode::BadRequest,
									"Invalid file name".to_string(),
								));
							}

							let Some(parent) = old_path.parent() else {
								return Err(rspc::Error::new(
									ErrorCode::BadRequest,
									"Missing parent path on file to be renamed".to_string(),
								));
							};

							let new_path = parent.join(replaced_full_name.as_ref());

							fs::rename(&old_path, &new_path).await.map_err(|e| {
								error!(
									old_path = %old_path.display(),
									new_path = %new_path.display(),
									?e,
									"Failed to rename file;",
								);
								let e = FileIOError::from((old_path, e, "Failed to rename file"));
								rspc::Error::with_cause(ErrorCode::Conflict, e.to_string(), e)
							})
						})
						.collect::<Vec<_>>()
						.try_join()
						.await?;

					Ok(())
				}
			}

			R.with2(library()).mutation(
				|(_, library), EphemeralRenameFileArgs { kind }: EphemeralRenameFileArgs| async move {
					let res = match kind {
						EphemeralRenameKind::One(one) => {
							EphemeralRenameFileArgs::rename_one(one).await
						}
						EphemeralRenameKind::Many(many) => {
							EphemeralRenameFileArgs::rename_many(many).await
						}
					};

					if res.is_ok() {
						invalidate_query!(library, "search.ephemeralPaths");
					}

					res
				},
			)
		})
}

#[derive(Type, Deserialize)]
struct EphemeralFileSystemOps {
	sources: Vec<PathBuf>,
	target_dir: PathBuf,
}

impl EphemeralFileSystemOps {
	async fn check_target_directory(&self) -> Result<(), rspc::Error> {
		match fs::metadata(&self.target_dir).await {
			Ok(metadata) => {
				if !metadata.is_dir() {
					return Err(rspc::Error::new(
						ErrorCode::BadRequest,
						"Target is not a directory".to_string(),
					));
				}
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				let e = FileIOError::from((&self.target_dir, e, "Target directory not found"));
				return Err(rspc::Error::with_cause(
					ErrorCode::BadRequest,
					e.to_string(),
					e,
				));
			}
			Err(e) => {
				return Err(FileIOError::from((
					&self.target_dir,
					e,
					"Failed to get target metadata",
				))
				.into());
			}
		}

		Ok(())
	}

	fn check_sources(&self) -> Result<(), rspc::Error> {
		if self.sources.is_empty() {
			return Err(rspc::Error::new(
				ErrorCode::BadRequest,
				"Sources cannot be empty".to_string(),
			));
		}

		Ok(())
	}

	async fn check(&self) -> Result<(), rspc::Error> {
		self.check_sources()?;
		self.check_target_directory().await?;

		Ok(())
	}

	async fn copy(self, library: &Library) -> Result<(), rspc::Error> {
		// dar um jeito de fazer o progress bar aparecer
		// levar em conta todos os arquivos
		//    pensando que a quantidade de arquivos muda naquele

		self.check().await?;

		let EphemeralFileSystemOps {
			sources,
			target_dir,
		} = self;

		let (directories_to_create, files_to_copy) = sources
			.into_iter()
			.filter_map(|source| {
				if let Some(name) = source.file_name() {
					let target = target_dir.join(name);
					Some((source, target))
				} else {
					warn!(source = %source.display(), "Skipping file with no name;");
					None
				}
			})
			.map(|(source, target)| async move {
				match fs::metadata(&source).await {
					Ok(metadata) => Ok((source, target, metadata.is_dir())),
					Err(e) => Err(FileIOError::from((
						source,
						e,
						"Failed to get source file metadata",
					))),
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?
			.into_iter()
			.partition::<Vec<_>, _>(|(_, _, is_dir)| *is_dir);

		let (foo, _report) = FileCopier::new(files_to_copy.into_iter().map(|(a, b, _)| (a, b)));
		foo.copy().await;

		if !directories_to_create.is_empty() {
			directories_to_create
				.into_iter()
				.map(|(source, mut target, _)| async move {
					match fs::metadata(&target).await {
						Ok(_) => target = find_available_filename_for_duplicate(&target).await?,
						Err(e) if e.kind() == io::ErrorKind::NotFound => {
							// Everything is awesome!
						}
						Err(e) => {
							return Err(rspc::Error::from(FileIOError::from((
								target,
								e,
								"Failed to get target file metadata",
							))));
						}
					}

					fs::create_dir_all(&target).await.map_err(|e| {
						FileIOError::from((&target, e, "Failed to create directory"))
					})?;

					let more_files =
						ReadDirStream::new(fs::read_dir(&source).await.map_err(|e| {
							FileIOError::from((&source, e, "Failed to read directory to be copied"))
						})?)
						.map(|read_dir| match read_dir {
							Ok(dir_entry) => Ok(dir_entry.path()),
							Err(e) => Err(FileIOError::from((
								&source,
								e,
								"Failed to read directory to be copied",
							))),
						})
						.collect::<Result<Vec<_>, _>>()
						.await?;

					if !more_files.is_empty() {
						Self {
							sources: more_files,
							target_dir: target,
						}
						.copy(library)
						.await
					} else {
						Ok(())
					}
				})
				.collect::<Vec<_>>()
				.try_join()
				.await?;
		}

		invalidate_query!(library, "search.ephemeralPaths");

		Ok(())
	}

	async fn cut(self, library: &Library) -> Result<(), rspc::Error> {
		self.check().await?;

		let EphemeralFileSystemOps {
			sources,
			target_dir,
		} = self;

		sources
			.into_iter()
			.filter_map(|source| {
				if let Some(name) = source.file_name() {
					let target = target_dir.join(name);
					Some((source, target))
				} else {
					warn!(source = %source.display(), "Skipping file with no name;");
					None
				}
			})
			.map(|(source, target)| async move {
				match fs::metadata(&target).await {
					Ok(_) => {
						return Err(FileSystemJobsError::WouldOverwrite(
							target.into_boxed_path(),
						));
					}
					Err(e) if e.kind() == io::ErrorKind::NotFound => {
						// Everything is awesome!
					}
					Err(e) => {
						return Err(FileSystemJobsError::FileIO(FileIOError::from((
							source,
							e,
							"Failed to get target file metadata",
						))));
					}
				}

				fs::rename(&source, target).await.map_err(|e| {
					FileSystemJobsError::FileIO(FileIOError::from((
						source,
						e,
						"Failed to move file",
					)))
				})
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		invalidate_query!(library, "search.ephemeralPaths");

		Ok(())
	}
}

#[derive(Debug)]
struct FileCopy {
	source: PathBuf,
	destiny: PathBuf,
}

#[derive(Debug)]
enum Progress {
	Started { file: PathBuf, total_size: u64 },
	Advanced { file: PathBuf, partial_size: u64 },
	Finished(Result<PathBuf, PathBuf>),
}

#[derive(Debug)]
pub struct FileCopier {
	map: Vec<FileCopy>,
	progress: async_channel::Sender<Progress>,
}

impl FileCopier {
	#[must_use = "creating a FileCopier does nothing unless called"]
	pub fn new(map: impl IntoIterator<Item = (PathBuf, PathBuf)>) -> (Self, Receiver<Progress>) {
		let map = map
			.into_iter()
			.map(|(source, destiny)| FileCopy { source, destiny })
			.collect();
		let (progress, report) = async_channel::unbounded();

		{
			let report = report.clone();
			tokio::spawn(async move {
				use std::collections::HashMap;
				let mut interval = tokio::time::interval(Duration::from_micros(50));
				let mut files = HashMap::new();
				loop {
					interval.tick().await;
					match report.recv().await {
						Ok(Progress::Started { file, total_size }) => {
							tracing::debug!(?file, ?total_size, "started progressing");
							files.insert(file, total_size);
						}
						Ok(Progress::Advanced { file, partial_size }) => {
							let total = files.get(&file).copied().unwrap() as f64;
							let percetage =
								((partial_size as f64 / total) * 100.0f64).floor() as u64;
							tracing::debug!(?file, partial_size, percetage, "progress");
						}
						Ok(Progress::Finished(Ok(file))) => {
							tracing::debug!(?file, "completed");
							// break;
						}
						Ok(Progress::Finished(Err(file))) => {
							tracing::debug!(?file, "file erroed");
							// break;
						}
						Err(_) => {
							//tracing::error!(error=?e, "wtf");
						}
					}
				}
			});
		}

		(Self { map, progress }, report)
	}

	#[tracing::instrument(skip_all)]
	pub async fn copy(self) -> Result<(), rspc::Error> {
		self.map
			.into_iter()
			.map(|mut file| {
				let report = self.progress.clone();

				async move {
					let new_name = match fs::try_exists(&file.destiny).await {
						Ok(true) => find_available_filename_for_duplicate(&file.destiny).await,
						Ok(false) => Ok(file.destiny),
						Err(_) => todo!(), //  TODO(matheus-consoli):  todo!()
					};
					file.destiny = new_name.unwrap();

					let source = file.source.clone();

					let destiny = file.destiny.clone();

					let source_size = fs::metadata(&file.source).await.unwrap().len();
					report
						.send(Progress::Started {
							file: file.destiny.clone(),
							total_size: source_size,
						})
						.await;

					let copy = tokio::spawn(fs::copy(source, destiny));

					let check = tokio::spawn(async move {
						let mut interval = tokio::time::interval(Duration::from_micros(100));

						loop {
							interval.tick().await;
							tokio::time::sleep(Duration::from_micros(30)).await;
							match fs::metadata(&file.destiny).await {
								Ok(metadata) => {
									let len = metadata.len();
									if len == source_size {
										_ = report
											.send(Progress::Finished(Ok(file.destiny.clone())))
											.await;
										break;
									}
									_ = report
										.send(Progress::Advanced {
											file: file.destiny.clone(),
											partial_size: len,
										})
										.await;
								}
								Err(_) => (),
							}
						}
						Ok::<_, io::Error>(())
					});

					(copy, check).try_join().await;
				}
			})
			.collect::<Vec<_>>()
			.join()
			.await;

		Ok(())
	}
}
