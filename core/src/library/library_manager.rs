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
	invalidate_query,
	node::Platform,
	prisma::{self, node},
	util::db::load_and_migrate,
	NodeContext,
};

use super::{LibraryConfig, LibraryConfigWrapped, LibraryContext};

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
	IO(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	Json(#[from] serde_json::Error),
	#[error("Database error: {0}")]
	Database(#[from] prisma::QueryError),
	#[error("Library not found error")]
	LibraryNotFound,
	#[error("error migrating the config file")]
	Migration(String),
	#[error("failed to parse uuid")]
	Uuid(#[from] uuid::Error),
	#[error("error opening database as the path contains non-UTF-8 characters")]
	InvalidDatabasePath(PathBuf),
}

impl From<LibraryManagerError> for rspc::Error {
	fn from(error: LibraryManagerError) -> Self {
		rspc::Error::new(rspc::ErrorCode::InternalServerError, error.to_string())
	}
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
				.map(|v| v.to_str().map(Uuid::from_str))
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
			libraries.push(Self::load(library_id, &db_path, config, node_context.clone()).await?);
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
			.await?;
		}

		Ok(this)
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(&self, config: LibraryConfig) -> Result<(), LibraryManagerError> {
		let id = Uuid::new_v4();
		LibraryConfig::save(
			Path::new(&self.libraries_dir).join(format!("{id}.sdlibrary")),
			&config,
		)
		.await?;

		let library = Self::load(
			id,
			self.libraries_dir.join(format!("{id}.db")),
			config,
			self.node_context.clone(),
		)
		.await?;

		invalidate_query!(library, "library.get": (), ());

		self.libraries.write().await.push(library);
		Ok(())
	}

	pub(crate) async fn get_all_libraries_config(&self) -> Vec<LibraryConfigWrapped> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|lib| LibraryConfigWrapped {
				config: lib.config.clone(),
				uuid: lib.id,
			})
			.collect()
	}

	pub(crate) async fn get_all_libraries_ctx(&self) -> Vec<LibraryContext> {
		self.libraries.read().await.clone()
	}

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<String>,
		description: Option<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let mut libraries = self.libraries.write().await;
		let library = libraries
			.iter_mut()
			.find(|lib| lib.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		// update the library
		if let Some(name) = name {
			library.config.name = name;
		}
		if let Some(description) = description {
			library.config.description = description;
		}

		LibraryConfig::save(
			Path::new(&self.libraries_dir).join(format!("{id}.sdlibrary")),
			&library.config,
		)
		.await?;

		invalidate_query!(library, "library.get": (), ());

		Ok(())
	}

	pub async fn delete_library(&self, id: Uuid) -> Result<(), LibraryManagerError> {
		let mut libraries = self.libraries.write().await;

		let library = libraries
			.iter()
			.find(|l| l.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		fs::remove_file(Path::new(&self.libraries_dir).join(format!("{}.db", library.id)))?;
		fs::remove_file(Path::new(&self.libraries_dir).join(format!("{}.sdlibrary", library.id)))?;

		invalidate_query!(library, "library.get": (), ());

		libraries.retain(|l| l.id != id);

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub(crate) async fn get_ctx(&self, library_id: Uuid) -> Option<LibraryContext> {
		self.libraries
			.read()
			.await
			.iter()
			.find(|lib| lib.id == library_id)
			.map(Clone::clone)
	}

	/// load the library from a given path
	pub(crate) async fn load(
		id: Uuid,
		db_path: impl AsRef<Path>,
		config: LibraryConfig,
		node_context: NodeContext,
	) -> Result<LibraryContext, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let db = Arc::new(
			load_and_migrate(&format!(
				"file:{}",
				db_path.as_os_str().to_str().ok_or_else(|| {
					LibraryManagerError::InvalidDatabasePath(db_path.to_path_buf())
				})?
			))
			.await
			.unwrap(),
		);

		let node_config = node_context.config.get().await;

		let platform = match env::consts::OS {
			"windows" => Platform::Windows,
			"macos" => Platform::MacOS,
			"linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		let uuid_vec = id.as_bytes().to_vec();

		let node_data = db
			.node()
			.upsert(
				node::pub_id::equals(uuid_vec.clone()),
				(
					uuid_vec,
					node_config.name.clone(),
					vec![node::platform::set(platform as i32)],
				),
				vec![node::name::set(node_config.name.clone())],
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
