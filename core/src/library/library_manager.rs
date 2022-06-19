use std::{
	env, fs, io,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
	node::Platform,
	prisma::{self, library, node},
	util::db::load_and_migrate,
	NodeContext,
};

use super::{LibraryConfig, LibraryContext};

/// LibraryManager is a singleton that manages all libraries for a node.
pub struct LibraryManager {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<Vec<LibraryContext>>,
	/// node_context holds the context for the node which this library manager is running on.
	node_context: NodeContext,
}

#[derive(Error, Debug)]
pub enum LibraryManagerError {
	#[error("error saving or loading the config from the filesystem")]
	IOError(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	JsonError(#[from] serde_json::Error),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("error migrating the config file")]
	MigrationError(String),
}

impl LibraryManager {
	pub(crate) async fn new(
		libraries_dir: PathBuf,
		node_context: NodeContext,
	) -> Result<Arc<Self>, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir)?;

		let mut libraries = Vec::new();
		for entry in fs::read_dir(&libraries_dir)?
			.into_iter()
			.filter_map(|entry| entry.ok())
			.filter(|entry| {
				entry.path().is_file()
					&& entry
						.path()
						.extension()
						.map(|v| &*v == "sdlibrary")
						.unwrap_or(false)
			}) {
			let config_path = entry.path();
			let library_id = match Path::new(&config_path)
				.file_stem()
				.map(|v| v.to_str().map(|v| Uuid::from_str(v)))
			{
				Some(Some(Ok(id))) => id,
				_ => {
					println!("Attempted to load library from path '{}' but it has an invalid filename. Skipping...", config_path.display());
					continue;
				}
			};

			let db_path = config_path.clone().with_extension("db");
			if !db_path.exists() {
				println!(
					"Found library '{}' but no matching database file was found. Skipping...",
					config_path.display()
				);
				continue;
			}

			let config = LibraryConfig::read(config_path).await?;
			libraries.push(
				Self::load(
					library_id,
					db_path.to_str().unwrap(),
					config,
					node_context.clone(),
				)
				.await?,
			);
		}

		let this = Arc::new(Self {
			libraries: RwLock::new(libraries),
			libraries_dir,
			node_context,
		});

		// TODO: Remove this before merging PR -> Currently it exists to make the app usable
		if this.libraries.read().await.len() == 0 {
			this.create(LibraryConfig {
				name: "My Default Library".into(),
				..Default::default()
			})
			.await
			.unwrap();
		}

		Ok(this)
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(&self, config: LibraryConfig) -> Result<(), LibraryManagerError> {
		let id = Uuid::new_v4();
		LibraryConfig::save(
			Path::new(&self.libraries_dir).join(format!("{}.sdlibrary", id.to_string())),
			&config,
		)
		.await?;

		let name = config.name.clone();
		let library = Self::load(
			id,
			&Path::new(&self.libraries_dir)
				.join(format!("{}.db", id.to_string()))
				.to_str()
				.unwrap(),
			config,
			self.node_context.clone(),
		)
		.await?;

		library
			.db
			.library()
			.create(
				library::pub_id::set(library.id.to_string()),
				library::name::set(name),
				vec![],
			)
			.exec()
			.await?;

		self.libraries.write().await.push(library);

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	// TODO: Return the context based on a library_id. This currently only returns the first because the UI isn't ready for multi-library support yet.
	pub(crate) async fn get_ctx(&self) -> Option<LibraryContext> {
		self.libraries.read().await.first().map(|v| v.clone())
	}

	/// load the library from a given path
	pub(crate) async fn load(
		id: Uuid,
		db_path: &str,
		config: LibraryConfig,
		node_context: NodeContext,
	) -> Result<LibraryContext, LibraryManagerError> {
		let db = Arc::new(
			load_and_migrate(&format!("file:{}", db_path))
				.await
				.unwrap(),
		);

		let platform = match env::consts::OS {
			"windows" => Platform::Windows,
			"macos" => Platform::MacOS,
			"linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		let node_data = db
			.node()
			.upsert(
				node::pub_id::equals(id.to_string()),
				(
					node::pub_id::set(id.to_string()),
					node::name::set(config.name.clone()),
					vec![node::platform::set(platform as i32)],
				),
				vec![node::name::set(config.name.clone())],
			)
			.exec()
			.await?;

		Ok(LibraryContext {
			id,
			config,
			db,
			node_local_id: node_data.id,
			node_context,
		})
	}
}
