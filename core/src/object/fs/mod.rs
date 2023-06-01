use crate::{
	location::{
		file_path_helper::{file_path_with_object, FilePathId, IsolatedFilePathData},
		LocationError, LocationId,
	},
	prisma::{file_path, location, PrismaClient},
};

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod create;
pub mod delete;
pub mod erase;

pub mod copy;
pub mod cut;

pub mod decrypt;
pub mod encrypt;

pub mod error;

use error::FileSystemJobsError;

pub const BYTES_EXT: &str = ".bytes";

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

pub async fn get_location_path_from_location_id(
	db: &PrismaClient,
	location_id: FilePathId,
) -> Result<PathBuf, FileSystemJobsError> {
	db.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.map(|location| PathBuf::from(location.path))
		.ok_or(FileSystemJobsError::Location(LocationError::IdNotFound(
			location_id,
		)))
}

pub async fn get_many_files_datas(
	db: &PrismaClient,
	location_path: impl AsRef<Path>,
	file_path_ids: &[FilePathId],
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
			.map(|path_data| FileData {
				full_path: location_path.join(IsolatedFilePathData::from(&path_data)),
				file_path: path_data,
			})
	})
	.collect::<Result<Vec<_>, _>>()
}

pub async fn get_file_data_from_isolated_file_path(
	db: &PrismaClient,
	location_path: impl AsRef<Path>,
	iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<FileData, FileSystemJobsError> {
	db.file_path()
		.find_unique(iso_file_path.into())
		.include(file_path_with_object::include())
		.exec()
		.await?
		.ok_or_else(|| {
			FileSystemJobsError::FilePathNotFound(
				AsRef::<Path>::as_ref(iso_file_path)
					.to_path_buf()
					.into_boxed_path(),
			)
		})
		.map(|path_data| FileData {
			full_path: location_path
				.as_ref()
				.join(IsolatedFilePathData::from(&path_data)),
			file_path: path_data,
		})
}

pub async fn fetch_source_and_target_location_paths(
	db: &PrismaClient,
	source_location_id: LocationId,
	target_location_id: LocationId,
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
			PathBuf::from(source_location.path),
			PathBuf::from(target_location.path),
		)),
		(None, _) => Err(FileSystemJobsError::Location(LocationError::IdNotFound(
			source_location_id,
		))),
		(_, None) => Err(FileSystemJobsError::Location(LocationError::IdNotFound(
			target_location_id,
		))),
	}
}

fn construct_target_filename(
	source_file_data: &FileData,
	target_file_name_suffix: &Option<String>,
) -> String {
	// extension wizardry for cloning and such
	// if no suffix has been selected, just use the file name
	// if a suffix is provided and it's a directory, use the directory name + suffix
	// if a suffix is provided and it's a file, use the (file name + suffix).extension

	if let Some(ref suffix) = target_file_name_suffix {
		if source_file_data.file_path.is_dir {
			format!("{}{suffix}", source_file_data.file_path.name)
		} else {
			format!(
				"{}{suffix}.{}",
				source_file_data.file_path.name, source_file_data.file_path.extension,
			)
		}
	} else if source_file_data.file_path.is_dir {
		source_file_data.file_path.name.clone()
	} else {
		format!(
			"{}.{}",
			source_file_data.file_path.name, source_file_data.file_path.extension
		)
	}
}
