pub mod locations;
pub mod volumes;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SysError {
    #[error("Location error")]
    LocationError(#[from] locations::LocationError),
    #[error("Error with system volumes")]
    VolumeError(String),
}
