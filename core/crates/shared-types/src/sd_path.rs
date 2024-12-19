use std::path::{Path, PathBuf};

use sd_prisma::prisma::{device, location, PrismaClient};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use uuid::Uuid;

pub type DeviceId = Uuid;
pub type LocationId = i32;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SdPath {
	device: DeviceId,
	location: Option<LocationId>,
	local_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum SdPathError {
	#[error("Device not found: {0}")]
	DeviceNotFound(DeviceId),
	#[error("Location not found: {0}")]
	LocationNotFound(LocationId),
	#[error("Path not found in location: {0}")]
	PathNotFound(PathBuf),
	#[error("Database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
}

impl SdPath {
	/// Create a new SdPath for a local file or directory
	pub fn local(device: DeviceId, path: impl Into<PathBuf>) -> Self {
		Self {
			device,
			location: None,
			local_path: path.into(),
		}
	}

	/// Create a new SdPath for a file or directory in a location
	pub fn new(device: DeviceId, location: LocationId, path: impl Into<PathBuf>) -> Self {
		Self {
			device,
			location: Some(location),
			local_path: path.into(),
		}
	}

	/// Validate that this path exists and is accessible
	pub async fn validate(&self, db: &PrismaClient) -> Result<(), SdPathError> {
		// Check device exists
		db.device()
			.find_unique(device::pub_id::equals(self.device.as_bytes().to_vec()))
			.exec()
			.await?
			.ok_or_else(|| SdPathError::DeviceNotFound(self.device))?;

		// If location is specified, check it exists
		if let Some(location_id) = self.location {
			db.location()
				.find_unique(location::id::equals(location_id))
				.exec()
				.await?
				.ok_or_else(|| SdPathError::LocationNotFound(location_id))?;

			// TODO: Check path exists in location's index
		} else {
			// For local paths, just check the file exists
			if !self.local_path.exists() {
				return Err(SdPathError::PathNotFound(self.local_path.clone()));
			}
		}

		Ok(())
	}

	/// Get the absolute path on the local filesystem
	pub async fn resolve(&self, db: &PrismaClient) -> Result<PathBuf, SdPathError> {
		if let Some(location_id) = self.location {
			// Get location path from database
			let location = db
				.location()
				.find_unique(location::id::equals(location_id))
				.exec()
				.await?
				.ok_or_else(|| SdPathError::LocationNotFound(location_id))?;

			// Join location path with local path
			Ok(Path::new(&location.path.unwrap_or_default()).join(&self.local_path))
		} else {
			// Local path is already absolute
			Ok(self.local_path.clone())
		}
	}

	// /// Returns true if this path is on the current device
	// pub fn is_local(&self) -> bool {
	// 	self.device == device::current_id() // TODO: Get current device ID
	// }

	/// Returns true if this path is in an indexed location
	pub fn is_indexed(&self) -> bool {
		self.location.is_some()
	}

	/// Returns true if the path exists on the local filesystem
	pub fn exists(&self) -> bool {
		self.local_path.exists()
	}

	/// Returns true if the path exists at its location
	/// If the path is not in a location, this is equivalent to exists()
	pub async fn exists_at_location(&self, db: &PrismaClient) -> Result<bool, SdPathError> {
		if self.is_indexed() {
			let resolved = self.resolve(db).await?;
			Ok(resolved.exists())
		} else {
			Ok(self.exists())
		}
	}

	/// Returns true if the path is a directory on the local filesystem
	pub fn is_dir(&self) -> bool {
		self.local_path.is_dir()
	}

	/// Returns true if the path is a directory at its location
	/// If the path is not in a location, this is equivalent to is_dir()
	pub async fn is_dir_at_location(&self, db: &PrismaClient) -> Result<bool, SdPathError> {
		if self.is_indexed() {
			let resolved = self.resolve(db).await?;
			Ok(resolved.is_dir())
		} else {
			Ok(self.is_dir())
		}
	}

	// Getters
	pub fn device(&self) -> DeviceId {
		self.device
	}

	pub fn location(&self) -> Option<LocationId> {
		self.location
	}

	pub fn local_path(&self) -> &Path {
		&self.local_path
	}
}
