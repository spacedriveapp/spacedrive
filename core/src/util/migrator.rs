use std::{
	any::type_name,
	fs::File,
	io::{self, BufReader, Seek, Write},
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
#[async_trait::async_trait]
pub trait Migrate: Sized + DeserializeOwned + Serialize {
	const CURRENT_VERSION: u32;

	type Ctx: Sync;

	fn default(path: PathBuf) -> Result<Self, MigratorError>;

	async fn migrate(
		from_version: u32,
		config: &mut Map<String, Value>,
		ctx: &Self::Ctx,
	) -> Result<(), MigratorError>;

	async fn load_and_migrate(path: &Path, ctx: &Self::Ctx) -> Result<Self, MigratorError> {
		match path.try_exists()? {
			true => {
				let mut file = File::options().read(true).write(true).open(path)?;
				let mut cfg: BaseConfig = match serde_json::from_reader(BufReader::new(&mut file)) {
					Ok(cfg) => cfg,
					Err(err) => {
						// This is for backwards compatibility for the backwards compatibility cause the super super old system store the version as a string.
						{
							file.rewind()?;
							let mut y = match serde_json::from_reader::<_, Value>(BufReader::new(
								&mut file,
							)) {
								Ok(y) => y,
								Err(_) => {
									return Err(err.into());
								}
							};

							if let Some(obj) = y.as_object_mut() {
								if obj.contains_key("version") {
									return Err(MigratorError::HasSuperLegacyConfig); // This is just to make the error nicer
								} else {
									return Err(err.into());
								}
							} else {
								return Err(err.into());
							}
						}
					}
				};
				file.rewind()?; // Fail early so we don't end up invalid state

				if cfg.version > Self::CURRENT_VERSION {
					return Err(MigratorError::YourAppIsOutdated);
				}

				let is_latest = cfg.version == Self::CURRENT_VERSION;
				for v in (cfg.version + 1)..=Self::CURRENT_VERSION {
					cfg.version = v;
					match Self::migrate(v, &mut cfg.other, ctx).await {
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
				Self::default(path.into())?.save(path)?.other,
			))?),
		}
	}

	fn save(&self, path: &Path) -> Result<BaseConfig, MigratorError> {
		let config = BaseConfig {
			version: Self::CURRENT_VERSION,
			other: match serde_json::to_value(self)? {
				Value::Object(map) => map,
				_ => {
					return Err(MigratorError::InvalidType(type_name::<Self>()));
				}
			},
		};

		let mut file = File::create(path)?;
		file.write_all(serde_json::to_string(&config)?.as_bytes())?;
		Ok(config)
	}
}

#[derive(Error, Debug)]
pub enum MigratorError {
	#[error("Io - error saving or loading the config from the filesystem: {0}")]
	Io(#[from] io::Error),
	#[error("Json - error serializing or deserializing the JSON in the config file: {0}")]
	Json(#[from] serde_json::Error),
	#[error(
		"YourAppIsOutdated - the config file is for a newer version of the app. Please update to the latest version to load it!"
	)]
	YourAppIsOutdated,
	#[error("Type '{0}' as generic `Migrator::T` must be serialiable to a Serde object!")]
	InvalidType(&'static str),
	#[error("{0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("We detected a Spacedrive config from a super early version of the app!")]
	HasSuperLegacyConfig,
	#[error("file '{}' was not found by the migrator!", .0.display())]
	ConfigFileMissing(PathBuf),
	#[error("custom migration error: {0}")]
	Custom(String),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod test {
	use std::{fs, io::Read, path::PathBuf};

	use serde_json::json;

	use super::*;

	#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
	struct MyConfigType {
		// For testing add new fields without breaking the passing.
		#[serde(flatten)]
		other: Map<String, Value>,
	}

	#[async_trait::async_trait]
	impl Migrate for MyConfigType {
		const CURRENT_VERSION: u32 = 3;

		type Ctx = ();

		fn default(_path: PathBuf) -> Result<Self, MigratorError> {
			Ok(<Self as Default>::default())
		}

		async fn migrate(
			to_version: u32,
			config: &mut Map<String, Value>,
			_ctx: &Self::Ctx,
		) -> Result<(), MigratorError> {
			match to_version {
				0 => Ok(()),
				1 => {
					config.insert("a".into(), json!({}));
					Ok(())
				}
				2 => {
					config
						.get_mut("a")
						.and_then(|v| v.as_object_mut())
						.map(|v| v.insert("b".into(), json!({})));

					Ok(())
				}
				3 => {
					config
						.get_mut("a")
						.and_then(|v| v.as_object_mut())
						.and_then(|v| v.get_mut("b"))
						.and_then(|v| v.as_object_mut())
						.map(|v| v.insert("c".into(), json!("it works")));

					Ok(())
				}
				v => unreachable!("Missing migration for library version {}", v),
			}
		}
	}

	fn path(file_name: &'static str) -> PathBuf {
		let dir = PathBuf::from("./migration_test");
		std::fs::create_dir(&dir).ok();
		dir.join(file_name)
	}

	fn file_as_str(path: &Path) -> String {
		let mut file = File::open(path).unwrap();
		let mut contents = String::new();
		file.read_to_string(&mut contents).unwrap();
		contents
	}

	fn write_to_file(path: &Path, contents: &str) {
		let mut file = File::create(path).unwrap();
		file.write_all(contents.as_bytes()).unwrap();
	}

	#[tokio::test]
	async fn test_migrator_happy_path() {
		let p = path("test_migrator_happy_path.config");

		// Check config is created when it's missing
		assert!(!p.exists(), "config file should start out deleted");
		std::fs::write(
			&p,
			serde_json::to_string(&json!({
				"version": 0
			}))
			.unwrap(),
		)
		.unwrap();
		assert!(p.exists(), "config file was not initialised");
		assert_eq!(file_as_str(&p), r#"{"version":0}"#);

		// Load + migrate config
		let _config = MyConfigType::load_and_migrate(&p, &()).await.unwrap();

		assert_eq!(
			file_as_str(&p),
			r#"{"version":3,"a":{"b":{"c":"it works"}}}"#
		);

		// Cleanup
		fs::remove_file(&p).unwrap();
	}

	#[tokio::test]
	pub async fn test_time_traveling_backwards() {
		let p = path("test_time_traveling_backwards.config");

		// You opened a new database in an older version of the app
		write_to_file(&p, r#"{"version":5}"#);
		match MyConfigType::load_and_migrate(&p, &()).await {
			Err(MigratorError::YourAppIsOutdated) => (),
			_ => panic!("Should have failed to load config from a super newer version of the app"),
		}

		// Cleanup
		fs::remove_file(&p).unwrap();
	}
}
