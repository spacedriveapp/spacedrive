use std::{
	any::type_name,
	fmt::Display,
	future::Future,
	num::ParseIntError,
	path::{Path, PathBuf},
	str::FromStr,
};

use int_enum::IntEnum;
use itertools::Itertools;
use thiserror::Error;
use tokio::{fs, io};
use tracing::info;

use super::error::FileIOError;

#[derive(Error, Debug)]
pub enum VersionManagerError {
	#[error("invalid version")]
	InvalidVersion,
	#[error("version file does not exist")]
	VersionFileDoesNotExist,
	#[error("error while converting integer to enum")]
	IntConversionError,
	#[error("malformed version file")]
	MalformedVersionFile,
	#[error("unexpected migration: {current_version} -> {next_version}")]
	UnexpectedMigration {
		current_version: i32,
		next_version: i32,
	},

	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	ParseIntError(#[from] ParseIntError),
}

pub trait ManagedVersion: IntEnum<Int = i32> + Display + 'static {
	const LATEST_VERSION: Self;
	type MigrationError: std::error::Error + Display + From<VersionManagerError> + 'static;
}

/// An abstract system for saving a text file containing a version number.
/// The version number is an integer that can be converted to and from an enum.
/// The enum must implement the IntEnum trait.
pub struct VersionManager<T: ManagedVersion> {
	version_file_path: PathBuf,
	_marker: std::marker::PhantomData<T>,
}

impl<T: ManagedVersion> VersionManager<T> {
	pub fn new(version_file_path: impl AsRef<Path>) -> Self {
		VersionManager {
			version_file_path: version_file_path.as_ref().into(),
			_marker: std::marker::PhantomData,
		}
	}

	pub async fn get_version(&self) -> Result<T, VersionManagerError> {
		match fs::read_to_string(&self.version_file_path).await {
			Ok(contents) => {
				let version = i32::from_str(contents.trim())?;
				T::from_int(version).map_err(|_| VersionManagerError::IntConversionError)
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				Err(VersionManagerError::VersionFileDoesNotExist)
			}
			Err(e) => Err(FileIOError::from((&self.version_file_path, e)).into()),
		}
	}

	pub async fn set_version(&self, version: T) -> Result<(), VersionManagerError> {
		fs::write(
			&self.version_file_path,
			version.int_value().to_string().as_bytes(),
		)
		.await
		.map_err(|e| FileIOError::from((&self.version_file_path, e)).into())
	}

	pub async fn migrate<Fut>(
		&self,
		current: T,
		migrate_fn: impl Fn(T, T) -> Fut,
	) -> Result<(), T::MigrationError>
	where
		Fut: Future<Output = Result<(), T::MigrationError>>,
	{
		for (current_version, next_version) in
			(current.int_value()..=T::LATEST_VERSION.int_value()).tuple_windows()
		{
			match (T::from_int(current_version), T::from_int(next_version)) {
				(Ok(current), Ok(next)) => {
					info!(
						"Running {} migrator: {} -> {}",
						type_name::<T>(),
						current,
						next
					);
					migrate_fn(current, next).await?
				}
				(Err(_), _) | (_, Err(_)) => {
					return Err(VersionManagerError::IntConversionError.into())
				}
			};
		}

		self.set_version(T::LATEST_VERSION)
			.await
			.map_err(Into::into)
	}
}
