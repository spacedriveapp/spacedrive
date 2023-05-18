use std::{
	any::type_name,
	fs::File,
	io::{self, BufReader, Seek, Write},
	marker::PhantomData,
	path::Path,
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

	pub fn load(&self, path: &Path) -> Result<T, MigratorError> {
		match path.try_exists().unwrap() {
			true => {
				let mut file = File::options().read(true).write(true).open(path)?;
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
				self.save(path, T::default())?.other,
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

		let mut file = File::create(path)?;
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

#[cfg(test)]
mod test {
	use std::{fs, io::Read, path::PathBuf};

	use futures::executor::block_on;

	use super::*;

	#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
	pub struct MyConfigType {
		a: u8,
		// For testing add new fields without breaking the passing.
		#[serde(flatten)]
		other: Map<String, Value>,
	}

	pub fn migration_node(
		version: u32,
		config: &mut Map<String, Value>,
	) -> Result<(), MigratorError> {
		match version {
			0 => Ok(()),
			// Add field to config
			1 => {
				config.insert("b".into(), 2.into());
				Ok(())
			}
			// Async migration
			2 => {
				let mut a = false;
				block_on(async {
					a = true;
					config.insert("c".into(), 3.into());
				});
				assert!(a, "Async block was not blocked on correctly!");
				Ok(())
			}
			v => unreachable!("Missing migration for library version {}", v),
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

	#[test]
	fn test_migrator_happy_path() {
		let migrator = FileMigrator::<MyConfigType> {
			current_version: 0,
			migration_fn: migration_node,
			phantom: PhantomData,
		};

		let p = path("test_migrator_happy_path.config");

		// Check config is created when it's missing
		assert!(!p.exists(), "config file should start out deleted");
		let default_cfg = migrator.load(&p).unwrap();
		assert!(p.exists(), "config file was not initialised");
		assert_eq!(file_as_str(&p), r#"{"version":0,"a":0}"#);

		// Check config can be loaded back into the system correctly
		let config = migrator.load(&p).unwrap();
		assert_eq!(default_cfg, config, "Config has got mangled somewhere");

		// Update the config and check it saved correctly
		let mut new_config = config;
		new_config.a = 1;
		migrator.save(&p, new_config.clone()).unwrap();
		assert_eq!(file_as_str(&p), r#"{"version":0,"a":1}"#);

		// Try loading in the new config and check it's correct
		let config = migrator.load(&p).unwrap();
		assert_eq!(
			new_config, config,
			"Config has got mangled during the saving process"
		);

		// Test upgrading to a new version which adds a field
		let migrator = FileMigrator::<MyConfigType> {
			current_version: 1,
			migration_fn: migration_node,
			phantom: PhantomData,
		};

		// Try loading in the new config and check it was updated
		let config = migrator.load(&p).unwrap();
		assert_eq!(file_as_str(&p), r#"{"version":1,"a":1,"b":2}"#);

		// Check editing works
		let mut new_config = config;
		new_config.a = 2;
		migrator.save(&p, new_config).unwrap();
		assert_eq!(file_as_str(&p), r#"{"version":1,"a":2,"b":2}"#);

		// Test upgrading to a new version which adds a field asynchronously
		let migrator = FileMigrator::<MyConfigType> {
			current_version: 2,
			migration_fn: migration_node,
			phantom: PhantomData,
		};

		// Try loading in the new config and check it was updated
		migrator.load(&p).unwrap();
		assert_eq!(file_as_str(&p), r#"{"version":2,"a":2,"b":2,"c":3}"#);

		// Cleanup
		fs::remove_file(&p).unwrap();
	}

	#[test]
	pub fn test_time_traveling_backwards() {
		let p = path("test_time_traveling_backwards.config");

		// You opened a new database in an older version of the app
		write_to_file(&p, r#"{"version":5,"a":1}"#);
		let migrator = FileMigrator::<MyConfigType> {
			current_version: 2,
			migration_fn: migration_node,
			phantom: PhantomData,
		};
		match migrator.load(&p) {
			Err(MigratorError::YourAppIsOutdated) => (),
			_ => panic!("Should have failed to load config from a super newer version of the app"),
		}

		// Cleanup
		fs::remove_file(&p).unwrap();
	}
}
