use sd_utils::error::FileIOError;

use std::{
	any::type_name, fmt::Display, future::Future, num::ParseIntError, path::Path, str::FromStr,
};

use int_enum::{IntEnum, IntEnumError};
use itertools::Itertools;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;
use tokio::{fs, io};
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum VersionManagerError<Version: IntEnum<Int = u64>> {
	#[error("version file does not exist")]
	VersionFileDoesNotExist,
	#[error("malformed version file, reason: {reason}")]
	MalformedVersionFile { reason: &'static str },
	#[error("unexpected migration: {current_version} -> {next_version}")]
	UnexpectedMigration {
		current_version: u64,
		next_version: u64,
	},
	#[error("failed to convert version to config file")]
	ConvertToConfig,

	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	ParseInt(#[from] ParseIntError),
	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	#[error(transparent)]
	IntConversion(#[from] IntEnumError<Version>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	PlainText,
	Json(&'static str), // Version field name!
}

pub trait ManagedVersion<Version: IntEnum<Int = u64> + Display + Eq + Serialize + DeserializeOwned>:
	Serialize + DeserializeOwned + 'static
{
	const LATEST_VERSION: Version;

	const KIND: Kind;

	type MigrationError: std::error::Error + Display + From<VersionManagerError<Version>> + 'static;

	fn from_latest_version() -> Option<Self> {
		None
	}
}

/// An abstract system for saving a text file containing a version number.
/// The version number is an integer that can be converted to and from an enum.
/// The enum must implement the IntEnum trait.
pub struct VersionManager<
	Config: ManagedVersion<Version>,
	Version: IntEnum<Int = u64> + Display + Eq + Serialize + DeserializeOwned,
> {
	_marker: std::marker::PhantomData<(Config, Version)>,
}

impl<
		Config: ManagedVersion<Version>,
		Version: IntEnum<Int = u64> + Display + Eq + Serialize + DeserializeOwned,
	> VersionManager<Config, Version>
{
	async fn get_version(
		&self,
		version_file_path: impl AsRef<Path>,
	) -> Result<Version, VersionManagerError<Version>> {
		let version_file_path = version_file_path.as_ref();

		match Config::KIND {
			Kind::PlainText => match fs::read_to_string(version_file_path).await {
				Ok(contents) => {
					let version = u64::from_str(contents.trim())?;
					Version::from_int(version).map_err(Into::into)
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					Err(VersionManagerError::VersionFileDoesNotExist)
				}
				Err(e) => Err(FileIOError::from((version_file_path, e)).into()),
			},
			Kind::Json(field) => match fs::read(version_file_path).await {
				Ok(bytes) => {
					let Some(version) = serde_json::from_slice::<Map<String, Value>>(&bytes)?
						.get(field)
						.and_then(|version| version.as_u64())
					else {
						return Err(VersionManagerError::MalformedVersionFile {
							reason: "missing version field",
						});
					};

					Version::from_int(version).map_err(Into::into)
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					Err(VersionManagerError::VersionFileDoesNotExist)
				}
				Err(e) => Err(FileIOError::from((version_file_path, e)).into()),
			},
		}
	}

	async fn set_version(
		&self,
		version_file_path: impl AsRef<Path>,
		version: Version,
	) -> Result<(), VersionManagerError<Version>> {
		let version_file_path = version_file_path.as_ref();

		match Config::KIND {
			Kind::PlainText => fs::write(
				version_file_path,
				version.int_value().to_string().as_bytes(),
			)
			.await
			.map_err(|e| FileIOError::from((version_file_path, e)).into()),

			Kind::Json(field) => {
				let mut data_value = serde_json::from_slice::<Map<String, Value>>(
					&fs::read(version_file_path)
						.await
						.map_err(|e| FileIOError::from((version_file_path, e)))?,
				)?;

				data_value.insert(String::from(field), json!(version.int_value()));

				fs::write(version_file_path, serde_json::to_vec(&data_value)?)
					.await
					.map_err(|e| FileIOError::from((version_file_path, e)).into())
			}
		}
	}

	pub async fn migrate_and_load<Fut>(
		version_file_path: impl AsRef<Path>,
		migrate_fn: impl Fn(Version, Version) -> Fut,
	) -> Result<Config, Config::MigrationError>
	where
		Fut: Future<Output = Result<(), Config::MigrationError>>,
	{
		let version_file_path = version_file_path.as_ref();

		let this = VersionManager {
			_marker: std::marker::PhantomData::<(Config, Version)>,
		};

		let current = match this.get_version(version_file_path).await {
			Ok(version) => version,
			Err(VersionManagerError::VersionFileDoesNotExist) => {
				warn!(
					config = %type_name::<Config>(),
					latest_version = %Config::LATEST_VERSION,
					"Config file for does not exist, trying to create a new one with latest version;",
				);

				let Some(latest_config) = Config::from_latest_version() else {
					return Err(VersionManagerError::VersionFileDoesNotExist.into());
				};

				fs::write(
					version_file_path,
					match Config::KIND {
						Kind::PlainText => Config::LATEST_VERSION
							.int_value()
							.to_string()
							.as_bytes()
							.to_vec(),
						Kind::Json(_) => serde_json::to_vec(&latest_config)
							.map_err(|e| VersionManagerError::SerdeJson(e))?,
					},
				)
				.await
				.map_err(|e| {
					VersionManagerError::FileIO(FileIOError::from((version_file_path, e)))
				})?;

				return Ok(latest_config);
			}
			Err(e) => return Err(e.into()),
		};

		if current != Config::LATEST_VERSION {
			for (current_version, next_version) in
				(current.int_value()..=Config::LATEST_VERSION.int_value()).tuple_windows()
			{
				let (current, next) = (
					Version::from_int(current_version).map_err(VersionManagerError::from)?,
					Version::from_int(next_version).map_err(VersionManagerError::from)?,
				);

				info!(
					config = %type_name::<Config>(),
					%current,
					%next,
					"Running migrator;",
				);
				migrate_fn(current, next).await?;
			}

			this.set_version(version_file_path, Config::LATEST_VERSION)
				.await?;
		} else {
			debug!(config = %type_name::<Config>(), "No migration required;");
		}

		fs::read(version_file_path)
			.await
			.map_err(|e| {
				VersionManagerError::FileIO(FileIOError::from((version_file_path, e))).into()
			})
			.and_then(|bytes| {
				serde_json::from_slice(&bytes).map_err(|e| VersionManagerError::SerdeJson(e).into())
			})
	}
}
