use rspc::ErrorCode;
use sd_core_file_path_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	FilePathError, IsolatedFilePathData,
};

use sd_prisma::prisma::{location, PrismaClient};

use std::path::{Path, PathBuf};

use prisma_client_rust::QueryError;

#[derive(thiserror::Error, Debug)]
pub enum SubPathError {
	#[error("received sub path not in database: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	#[error("database error: {0}")]
	Database(#[from] QueryError),

	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),
}

impl From<SubPathError> for rspc::Error {
	fn from(err: SubPathError) -> Self {
		match err {
			SubPathError::SubPathNotFound(_) => {
				Self::with_cause(ErrorCode::NotFound, err.to_string(), err)
			}

			_ => Self::with_cause(ErrorCode::InternalServerError, err.to_string(), err),
		}
	}
}

pub async fn get_full_path_from_sub_path(
	location_id: location::id::Type,
	sub_path: &Option<impl AsRef<Path> + Send + Sync>,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
) -> Result<PathBuf, SubPathError> {
	let location_path = location_path.as_ref();

	match sub_path {
		Some(sub_path) if sub_path.as_ref() != Path::new("") => {
			let sub_path = sub_path.as_ref();
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path).await?;

			ensure_sub_path_is_directory(location_path, sub_path).await?;

			ensure_file_path_exists(
				sub_path,
				&IsolatedFilePathData::new(location_id, location_path, &full_path, true)?,
				db,
				SubPathError::SubPathNotFound,
			)
			.await?;

			Ok(full_path)
		}
		_ => Ok(location_path.to_path_buf()),
	}
}

pub async fn maybe_get_iso_file_path_from_sub_path(
	location_id: location::id::Type,
	sub_path: &Option<impl AsRef<Path> + Send + Sync>,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
) -> Result<Option<IsolatedFilePathData<'static>>, SubPathError> {
	let location_path = location_path.as_ref();

	match sub_path {
		Some(sub_path) if sub_path.as_ref() != Path::new("") => {
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path).await?;
			ensure_sub_path_is_directory(location_path, sub_path).await?;

			let sub_iso_file_path =
				IsolatedFilePathData::new(location_id, location_path, &full_path, true)?;

			ensure_file_path_exists(
				sub_path,
				&sub_iso_file_path,
				db,
				SubPathError::SubPathNotFound,
			)
			.await
			.map(|()| Some(sub_iso_file_path))
		}
		_ => Ok(None),
	}
}
