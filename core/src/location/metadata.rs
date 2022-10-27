use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use uuid::Uuid;

static SPACEDRIVE_LOCATION_METADATA_FILE: &str = ".spacedrive";

pub(super) type LibraryId = Uuid;
pub(super) type LocationPubId = Uuid;

#[derive(Serialize, Deserialize, Default, Debug)]
struct LocationMetadata {
	pub_id: LocationPubId,
	name: String,
	path: PathBuf,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct SpacedriveLocationMetadata {
	libraries: HashMap<LibraryId, LocationMetadata>,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
}

pub(super) struct SpacedriveLocationMetadataFile {
	path: PathBuf,
	metadata: SpacedriveLocationMetadata,
}

impl SpacedriveLocationMetadataFile {
	pub(super) async fn try_load(
		location_path: impl AsRef<Path>,
	) -> Result<Option<Self>, LocationMetadataError> {
		let metadata_file_name = location_path
			.as_ref()
			.join(SPACEDRIVE_LOCATION_METADATA_FILE);

		match fs::read(&metadata_file_name).await {
			Ok(data) => Ok(Some(Self {
				path: metadata_file_name,
				metadata: serde_json::from_slice(&data).map_err(|e| {
					LocationMetadataError::Deserialize(e, location_path.as_ref().to_path_buf())
				})?,
			})),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
			Err(e) => Err(LocationMetadataError::Read(
				e,
				location_path.as_ref().to_path_buf(),
			)),
		}
	}

	pub(super) async fn create_and_save(
		library_id: LibraryId,
		location_pub_id: Uuid,
		location_path: impl AsRef<Path>,
		location_name: String,
	) -> Result<(), LocationMetadataError> {
		Self {
			path: location_path
				.as_ref()
				.join(SPACEDRIVE_LOCATION_METADATA_FILE),
			metadata: SpacedriveLocationMetadata {
				libraries: [(
					library_id,
					LocationMetadata {
						pub_id: location_pub_id,
						name: location_name,
						path: location_path.as_ref().to_path_buf(),
						created_at: Utc::now(),
						updated_at: Utc::now(),
					},
				)]
				.into_iter()
				.collect(),
				created_at: Utc::now(),
				updated_at: Utc::now(),
			},
		}
		.write_metadata()
		.await
	}

	pub(super) async fn relink(
		&mut self,
		library_id: LibraryId,
		location_path: impl AsRef<Path>,
	) -> Result<(), LocationMetadataError> {
		let location_metadata = self
			.metadata
			.libraries
			.get_mut(&library_id)
			.ok_or(LocationMetadataError::LibraryNotFound(library_id))?;

		let new_path = location_path.as_ref().to_path_buf();
		if location_metadata.path == new_path {
			return Err(LocationMetadataError::RelinkSamePath(new_path));
		}

		location_metadata.path = new_path;
		location_metadata.updated_at = Utc::now();
		self.path = location_path
			.as_ref()
			.join(SPACEDRIVE_LOCATION_METADATA_FILE);

		self.write_metadata().await
	}

	pub(super) async fn update(
		&mut self,
		library_id: LibraryId,
		location_name: String,
	) -> Result<(), LocationMetadataError> {
		let location_metadata = self
			.metadata
			.libraries
			.get_mut(&library_id)
			.ok_or(LocationMetadataError::LibraryNotFound(library_id))?;

		location_metadata.name = location_name;
		location_metadata.updated_at = Utc::now();

		self.write_metadata().await
	}

	pub(super) async fn add_library(
		&mut self,
		library_id: LibraryId,
		location_pub_id: Uuid,
		location_path: impl AsRef<Path>,
		location_name: String,
	) -> Result<(), LocationMetadataError> {
		self.metadata.libraries.insert(
			library_id,
			LocationMetadata {
				pub_id: location_pub_id,
				name: location_name,
				path: location_path.as_ref().to_path_buf(),
				created_at: Utc::now(),
				updated_at: Utc::now(),
			},
		);

		self.metadata.updated_at = Utc::now();
		self.write_metadata().await
	}

	pub(super) fn has_library(&self, library_id: LibraryId) -> bool {
		self.metadata.libraries.contains_key(&library_id)
	}

	pub(super) fn location_path(
		&self,
		library_id: LibraryId,
	) -> Result<&Path, LocationMetadataError> {
		self.metadata
			.libraries
			.get(&library_id)
			.map(|l| l.path.as_path())
			.ok_or(LocationMetadataError::LibraryNotFound(library_id))
	}

	pub(super) async fn remove_library(
		&mut self,
		library_id: LibraryId,
	) -> Result<(), LocationMetadataError> {
		self.metadata
			.libraries
			.remove(&library_id)
			.ok_or(LocationMetadataError::LibraryNotFound(library_id))?;

		self.metadata.updated_at = Utc::now();

		if !self.metadata.libraries.is_empty() {
			self.write_metadata().await
		} else {
			fs::remove_file(&self.path)
				.await
				.map_err(|e| LocationMetadataError::Delete(e, self.path.clone()))
		}
	}

	pub(super) fn location_pub_id(
		&self,
		library_id: LibraryId,
	) -> Result<Uuid, LocationMetadataError> {
		self.metadata
			.libraries
			.get(&library_id)
			.ok_or(LocationMetadataError::LibraryNotFound(library_id))
			.map(|m| m.pub_id)
	}

	async fn write_metadata(&self) -> Result<(), LocationMetadataError> {
		fs::write(
			&self.path,
			serde_json::to_vec(&self.metadata)
				.map_err(|e| LocationMetadataError::Serialize(e, self.path.clone()))?,
		)
		.await
		.map_err(|e| LocationMetadataError::Write(e, self.path.clone()))
	}
}

#[derive(Error, Debug)]
pub enum LocationMetadataError {
	#[error("Library not found: {0}")]
	LibraryNotFound(LibraryId),
	#[error("Failed to read location metadata file (path: {1:?}); (error: {0:?})")]
	Read(io::Error, PathBuf),
	#[error("Failed to delete location metadata file (path: {1:?}); (error: {0:?})")]
	Delete(io::Error, PathBuf),
	#[error("Failed to serialize metadata file for location (at path: {1:?}); (error: {0:?})")]
	Serialize(serde_json::Error, PathBuf),
	#[error("Failed to write location metadata file (path: {1:?}); (error: {0:?})")]
	Write(io::Error, PathBuf),
	#[error("Failed to deserialize metadata file for location (at path: {1:?}); (error: {0:?})")]
	Deserialize(serde_json::Error, PathBuf),
	#[error("Failed to relink, as the new location path is the same as the old path")]
	RelinkSamePath(PathBuf),
}
