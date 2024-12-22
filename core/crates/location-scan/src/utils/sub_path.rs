use sd_core_file_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	IsolatedFilePathData,
};

use sd_prisma::prisma::{location, PrismaClient};

use std::path::{Path, PathBuf};

use sd_core_shared_errors::job::sub_path::Error;

pub async fn get_full_path_from_sub_path<E: From<Error>>(
	location_id: location::id::Type,
	sub_path: Option<impl AsRef<Path> + Send + Sync>,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
) -> Result<PathBuf, E> {
	async fn inner(
		location_id: location::id::Type,
		sub_path: Option<&Path>,
		location_path: &Path,
		db: &PrismaClient,
	) -> Result<PathBuf, Error> {
		match sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(location_path, sub_path).await?;

				ensure_sub_path_is_directory(location_path, sub_path).await?;

				ensure_file_path_exists(
					sub_path,
					&IsolatedFilePathData::new(location_id, location_path, &full_path, true)?,
					db,
					Error::SubPathNotFound,
				)
				.await?;

				Ok(full_path)
			}
			_ => Ok(location_path.to_path_buf()),
		}
	}

	inner(
		location_id,
		sub_path.as_ref().map(AsRef::as_ref),
		location_path.as_ref(),
		db,
	)
	.await
	.map_err(E::from)
}

pub async fn maybe_get_iso_file_path_from_sub_path<E: From<Error>>(
	location_id: location::id::Type,
	sub_path: Option<impl AsRef<Path> + Send + Sync>,
	location_path: impl AsRef<Path> + Send,
	db: &PrismaClient,
) -> Result<Option<IsolatedFilePathData<'static>>, E> {
	async fn inner(
		location_id: location::id::Type,
		sub_path: Option<&Path>,
		location_path: &Path,
		db: &PrismaClient,
	) -> Result<Option<IsolatedFilePathData<'static>>, Error> {
		match sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(location_path, sub_path).await?;
				ensure_sub_path_is_directory(location_path, sub_path).await?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, location_path, &full_path, true)?;

				ensure_file_path_exists(sub_path, &sub_iso_file_path, db, Error::SubPathNotFound)
					.await
					.map(|()| Some(sub_iso_file_path))
			}
			_ => Ok(None),
		}
	}

	inner(
		location_id,
		sub_path.as_ref().map(AsRef::as_ref),
		location_path.as_ref(),
		db,
	)
	.await
	.map_err(E::from)
}
