pub mod locations;
pub mod volumes;
use thiserror::Error;

use crate::db;

use self::locations::LocationError;

#[derive(Error, Debug)]
pub enum SysError {
	#[error("Location error")]
	LocationError(#[from] LocationError),
	#[error("Error with system volumes")]
	VolumeError(String),
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}
