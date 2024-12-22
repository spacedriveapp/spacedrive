use thiserror::Error;

pub mod indexer_rules;
pub mod library;
pub mod location;
pub mod volume;
use sd_utils::error::FileIOError;

use tracing_subscriber::filter::FromEnvError;
// use sd_utils::version_manager::VersionManagerError;

/// Error type for Node related errors.
#[derive(Error, Debug)]
pub enum NodeError {
	#[error("NodeError::FailedToInitializeConfig({0})")]
	FailedToInitializeConfig(NodeConfigError),
	#[error("failed to initialize library manager: {0}")]
	FailedToInitializeLibraryManager(#[from] library::LibraryManagerError),
	#[error("failed to initialize location manager: {0}")]
	LocationManager(#[from] location::LocationManagerError),
	#[error("failed to initialize p2p manager: {0}")]
	P2PManager(String),
	#[error("invalid platform integer: {0}")]
	InvalidPlatformInt(u8),
	#[cfg(debug_assertions)]
	// #[error("init config error: {0}")]
	// InitConfig(#[from] util::debug_initializer::InitConfigError),
	#[error("logger error: {0}")]
	Logger(#[from] FromEnvError),
	#[error(transparent)]
	JobSystem(#[from] sd_core_job_errors::system::JobSystemError),
	#[error(transparent)]
	CloudServices(#[from] sd_core_cloud_services::Error),
	#[error(transparent)]
	Crypto(#[from] sd_crypto::Error),
	#[error(transparent)]
	Volume(#[from] volume::VolumeError),
	#[error(transparent)]
	PlatformDetection(#[from] PlatformDetectionError),
}

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	#[error(transparent)]
	VersionManager(#[from] VersionManagerError<Ver>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

#[derive(Debug, Error)]
pub enum PlatformDetectionError {
	#[error("invalid platform integer: {0}")]
	InvalidPlatformInt(u8),
}
