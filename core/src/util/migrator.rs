use std::{
	any::type_name,
	fs::File,
	io::{self, BufReader, Seek, Write},
	marker::PhantomData,
	path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;
use thiserror::Error;

/// is used to decode the configuration and work out what migrations need to be applied before the config can be properly loaded.
/// This allows us to migrate breaking changes to the config format between Spacedrive releases.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct BaseConfig {
	/// version of Spacedrive. Determined from `CARGO_PKG_VERSION` environment variable.
	pub version: u32,
	// Collect all extra fields
	#[serde(flatten)]
	other: Map<String, Value>,
}

/// System for managing app level migrations on a config file so we can introduce breaking changes to the app without the user needing to reset their whole system.
pub struct FileMigrator<T>
where
	T: Serialize + DeserializeOwned + Default,
{
	pub current_version: u32,
	pub migration_fn: fn(u32, &mut Map<String, Value>) -> Result<(), MigratorError>,
	pub phantom: PhantomData<T>,
}

impl<T> FileMigrator<T>
where
	T: Serialize + DeserializeOwned + Default,
{
	// TODO: This is blocked on Rust. Make sure to make all fields private when this is introduced! Tracking issue: https://github.com/rust-lang/rust/issues/57349
	// pub const fn new(
	// 	current_version: u32,
	// 	migration_fn: fn(u32, &mut Map<String, Value>) -> Result<(), MigratorError>,
	// ) -> Self {
	// 	Self {
	// 		current_version,
	// 		migration_fn,
	// 		phantom: PhantomData,
	// 	}
	// }

	pub fn load(&self, path: PathBuf) -> Result<T, MigratorError> {
		match path.try_exists().unwrap() {
			true => {
				let mut file = File::options().read(true).write(true).open(&path)?;
				let mut cfg: BaseConfig = serde_json::from_reader(BufReader::new(&mut file))?;
				file.rewind()?; // Fail early so we don't end up invalid state

				if cfg.version > self.current_version {
					return Err(MigratorError::YourAppIsOutdated);
				}

				let is_latest = cfg.version == self.current_version;
				for v in (cfg.version + 1)..=self.current_version {
					cfg.version = v;
					match (self.migration_fn)(v, &mut cfg.other) {
						Ok(()) => (),
						Err(err) => {
							file.write_all(serde_json::to_string(&cfg)?.as_bytes())?; // Writes updated version
							return Err(err);
						}
					}
				}

				if !is_latest {
					file.write_all(serde_json::to_string(&cfg)?.as_bytes())?; // Writes updated version
				}

				Ok(serde_json::from_value(Value::Object(cfg.other))?)
			}
			false => Ok(serde_json::from_value(Value::Object(
				self.save(&path, T::default())?.other,
			))?),
		}
	}

	pub fn save(&self, path: &Path, content: T) -> Result<BaseConfig, MigratorError> {
		let config = BaseConfig {
			version: self.current_version,
			other: match serde_json::to_value(content)? {
				Value::Object(map) => map,
				_ => {
					panic!(
						"Type '{}' as generic `Migrator::T` must be serialiable to a Serde object!",
						type_name::<T>()
					);
				}
			},
		};

		let mut file = File::create(&path)?;
		file.write_all(serde_json::to_string(&config)?.as_bytes())?;
		Ok(config)
	}
}

#[derive(Error, Debug)]
pub enum MigratorError {
	#[error("error saving or loading the config from the filesystem: {0}")]
	Io(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file: {0}")]
	Json(#[from] serde_json::Error),
	#[error(
		"the config file is for a newer version of the app. Please update to the latest version to load it!"
	)]
	YourAppIsOutdated,
	#[error("custom migration error: {0}")]
	Custom(String),
}
