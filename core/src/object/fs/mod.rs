use crate::location::LocationError;

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_with_object;

use sd_prisma::prisma::{file_path, location, PrismaClient};
use sd_utils::{
	db::maybe_missing,
	error::{FileIOError, NonUtf8PathError},
};
use tracing::trace;

use std::{
	ffi::OsStr,
	path::{Path, PathBuf},
	sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};

pub mod old_erase;

pub mod old_copy;
pub mod old_cut;

// pub mod decrypt;
// pub mod encrypt;

pub mod error;

use error::FileSystemJobsError;
use tokio::{fs, io};

static DUPLICATE_PATTERN: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r" \(\d+\)").expect("Failed to compile hardcoded regex"));

// pub const BYTES_EXT: &str = ".bytes";

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum ObjectType {
	File,
	Directory,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileData {
	pub file_path: file_path_with_object::Data,
	pub full_path: PathBuf,
}

/// Get the [`FileData`] related to every `file_path_id`
pub async fn get_many_files_datas(
	db: &PrismaClient,
	location_path: impl AsRef<Path>,
	file_path_ids: &[file_path::id::Type],
) -> Result<Vec<FileData>, FileSystemJobsError> {
	let location_path = location_path.as_ref();

	db._batch(
		file_path_ids
			.iter()
			.map(|file_path_id| {
				db.file_path()
					.find_unique(file_path::id::equals(*file_path_id))
					.include(file_path_with_object::include())
			})
			// FIXME:(fogodev -> Brendonovich) this collect is a workaround to a weird higher ranker lifetime error on
			// the _batch function, it should be removed once the error is fixed
			.collect::<Vec<_>>(),
	)
	.await?
	.into_iter()
	.zip(file_path_ids.iter())
	.map(|(maybe_file_path, file_path_id)| {
		maybe_file_path
			.ok_or(FileSystemJobsError::FilePathIdNotFound(*file_path_id))
			.and_then(|path_data| {
				Ok(FileData {
					full_path: location_path.join(IsolatedFilePathData::try_from(&path_data)?),
					file_path: path_data,
				})
			})
	})
	.collect()
}

pub async fn get_file_data_from_isolated_file_path(
	db: &PrismaClient,
	location_path: impl AsRef<Path>,
	iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<FileData, FileSystemJobsError> {
	let location_path = location_path.as_ref();
	db.file_path()
		.find_unique(iso_file_path.into())
		.include(file_path_with_object::include())
		.exec()
		.await?
		.ok_or_else(|| {
			FileSystemJobsError::FilePathNotFound(
				location_path.join(iso_file_path).into_boxed_path(),
			)
		})
		.and_then(|path_data| {
			Ok(FileData {
				full_path: location_path.join(IsolatedFilePathData::try_from(&path_data)?),
				file_path: path_data,
			})
		})
}

pub async fn fetch_source_and_target_location_paths(
	db: &PrismaClient,
	source_location_id: location::id::Type,
	target_location_id: location::id::Type,
) -> Result<(PathBuf, PathBuf), FileSystemJobsError> {
	match db
		._batch((
			db.location()
				.find_unique(location::id::equals(source_location_id)),
			db.location()
				.find_unique(location::id::equals(target_location_id)),
		))
		.await?
	{
		(Some(source_location), Some(target_location)) => Ok((
			maybe_missing(source_location.path.map(PathBuf::from), "location.path")?,
			maybe_missing(target_location.path.map(PathBuf::from), "location.path")?,
		)),
		(None, _) => Err(LocationError::IdNotFound(source_location_id))?,
		(_, None) => Err(LocationError::IdNotFound(target_location_id))?,
	}
}

fn construct_target_filename(source_file_data: &FileData) -> Result<String, FileSystemJobsError> {
	// extension wizardry for cloning and such
	// if no suffix has been selected, just use the file name
	// if a suffix is provided and it's a directory, use the directory name + suffix
	// if a suffix is provided and it's a file, use the (file name + suffix).extension

	Ok(
		if *maybe_missing(&source_file_data.file_path.is_dir, "file_path.is_dir")?
			|| source_file_data.file_path.extension.is_none()
			|| source_file_data.file_path.extension == Some(String::new())
		{
			maybe_missing(&source_file_data.file_path.name, "file_path.name")?.clone()
		} else {
			format!(
				"{}.{}",
				maybe_missing(&source_file_data.file_path.name, "file_path.name")?,
				maybe_missing(&source_file_data.file_path.extension, "file_path.extension")?
			)
		},
	)
}

pub fn append_digit_to_filename(
	final_path: &mut PathBuf,
	file_name: &str,
	ext: Option<&str>,
	current_int: u32,
) {
	let new_file_name = if let Some(found) = DUPLICATE_PATTERN.find_iter(file_name).last() {
		&file_name[..found.start()]
	} else {
		file_name
	}
	.to_string();

	if let Some(ext) = ext {
		final_path.push(format!("{} ({current_int}).{}", new_file_name, ext));
	} else {
		final_path.push(format!("{new_file_name} ({current_int})"));
	}
}

pub async fn find_available_filename_for_duplicate(
	target_path: impl AsRef<Path>,
) -> Result<PathBuf, FileSystemJobsError> {
	let target_path = target_path.as_ref();

	let new_file_name = target_path
		.file_stem()
		.ok_or_else(|| {
			FileSystemJobsError::MissingFileStem(target_path.to_path_buf().into_boxed_path())
		})?
		.to_str()
		.ok_or_else(|| NonUtf8PathError(target_path.to_path_buf().into_boxed_path()))?;

	let new_file_full_path_without_suffix =
		target_path.parent().map(Path::to_path_buf).ok_or_else(|| {
			FileSystemJobsError::MissingParentPath(target_path.to_path_buf().into_boxed_path())
		})?;

	for i in 1..u32::MAX {
		let mut new_file_full_path_candidate = new_file_full_path_without_suffix.clone();

		append_digit_to_filename(
			&mut new_file_full_path_candidate,
			new_file_name,
			target_path.extension().and_then(OsStr::to_str),
			i,
		);

		match fs::metadata(&new_file_full_path_candidate).await {
			Ok(_) => {
				// This candidate already exists, so we try the next one
				continue;
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				trace!(old_name=?target_path, new_name=?new_file_full_path_candidate, "duplicated file name, file renamed");
				return Ok(new_file_full_path_candidate);
			}
			Err(e) => return Err(FileIOError::from((new_file_full_path_candidate, e)).into()),
		}
	}

	Err(FileSystemJobsError::FailedToFindAvailableName(
		target_path.to_path_buf().into_boxed_path(),
	))
}
