pub mod locations;
pub mod volumes;
use thiserror::Error;

use crate::{db, job};

use self::locations::LocationError;

#[derive(Error, Debug)]
pub enum SysError {
	#[error("Location error")]
	LocationError(#[from] LocationError),
	#[error("Error with system volumes")]
	VolumeError(String),
	#[error("Error from job runner")]
	JobError(#[from] job::JobError),
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}
