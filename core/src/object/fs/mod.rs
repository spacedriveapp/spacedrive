use crate::{
	location::{
		file_path_helper::{file_path_with_object, IsolatedFilePathData},
		LocationError,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::{maybe_missing, MissingFieldError},
};

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod create;
pub mod delete;
pub mod erase;

pub mod copy;
pub mod cut;

// pub mod decrypt;
// pub mod encrypt;

pub mod error;

use error::FileSystemJobsError;

// pub const BYTES_EXT: &str = ".bytes";

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum ObjectType {
	File,
	Directory,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileData {
	pub file_path: file_path_with_object::Data,
	pub full_path: PathBuf,
}

pub async fn get_location_path_from_location_id(
	db: &PrismaClient,
	location_id: file_path::id::Type,
) -> Result<PathBuf, FileSystemJobsError> {
	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.ok_or(FileSystemJobsError::Location(LocationError::IdNotFound(
			location_id,
		)))?;

	Ok(maybe_missing(location.path, "location.path")?.into())
}

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
	.collect::<Result<Vec<_>, _>>()
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

fn construct_target_filename(
	source_file_data: &FileData,
	target_file_name_suffix: &Option<String>,
) -> Result<String, MissingFieldError> {
	// extension wizardry for cloning and such
	// if no suffix has been selected, just use the file name
	// if a suffix is provided and it's a directory, use the directory name + suffix
	// if a suffix is provided and it's a file, use the (file name + suffix).extension

	Ok(if let Some(ref suffix) = target_file_name_suffix {
		if maybe_missing(source_file_data.file_path.is_dir, "file_path.is_dir")?
			|| source_file_data.file_path.extension.is_none()
			|| source_file_data.file_path.extension == Some(String::new())
		{
			format!(
				"{}{suffix}",
				maybe_missing(&source_file_data.file_path.name, "file_path.name")?
			)
		} else {
			format!(
				"{}{suffix}.{}",
				maybe_missing(&source_file_data.file_path.name, "file_path.name")?,
				maybe_missing(&source_file_data.file_path.extension, "file_path.extension")?,
			)
		}
	} else if *maybe_missing(&source_file_data.file_path.is_dir, "file_path.is_dir")?
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
	})
}
