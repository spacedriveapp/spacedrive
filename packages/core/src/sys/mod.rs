pub mod locations;
pub mod volumes;
use thiserror::Error;

use crate::CoreError;

use self::locations::LocationError;

#[derive(Error, Debug)]
pub enum SysError {
    #[error("Location error")]
    LocationError(#[from] locations::LocationError),
    #[error("Error with system volumes")]
    VolumeError(String),
}

impl From<LocationError> for CoreError {
    fn from(e: LocationError) -> Self {
        CoreError::SysError(SysError::LocationError(e))
    }
}
