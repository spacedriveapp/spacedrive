use crate::indexer_rules;
use crate::location::LocationManagerError;
use sd_p2p::IdentityErr;
use sd_utils::{
	db::{self, MissingFieldError},
	error::{FileIOError, NonUtf8PathError},
	// version_manager::VersionManagerError,
};

type DevicePubId = uuid::Uuid;

use thiserror::Error;
use tracing::error;

#[derive(thiserror::Error, Debug)]
pub enum LibraryManagerError {
	#[error("error serializing or deserializing the JSON in the config file: {0}")]
	Json(#[from] serde_json::Error),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("library not found error")]
	LibraryNotFound,
	#[error("failed to parse uuid: {0}")]
	Uuid(#[from] uuid::Error),
	#[error("failed to run indexer rules seeder: {0}")]
	IndexerRulesSeeder(#[from] indexer_rules::SeederError),
	#[error("error migrating the library: {0}")]
	MigrationError(#[from] db::MigrationError),
	#[error("invalid library configuration: {0}")]
	InvalidConfig(String),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error("failed to watch locations: {0}")]
	LocationWatcher(#[from] LocationManagerError),
	#[error("failed to parse library p2p identity: {0}")]
	Identity(#[from] IdentityErr),
	#[error("failed to load private key for instance p2p identity")]
	InvalidIdentity,
	#[error("current instance with id '{0}' was not found in the database")]
	CurrentInstanceNotFound(String),
	#[error("current device with pub id '{0}' was not found in the database")]
	CurrentDeviceNotFound(DevicePubId),
	#[error("missing-field: {0}")]
	MissingField(#[from] MissingFieldError),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	LibraryConfig(#[from] LibraryConfigError),
	#[error(transparent)]
	CloudServices(#[from] sd_core_cloud_services::Error),
	#[error(transparent)]
	Sync(#[from] sd_core_library_sync::Error),
}

impl From<LibraryManagerError> for rspc::Error {
	fn from(error: LibraryManagerError) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			error.to_string(),
			error,
		)
	}
}

#[derive(Error, Debug)]
pub enum LibraryConfigError {
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("there are too many nodes in the database, this should not happen!")]
	TooManyNodes,
	#[error("there are too many instances in the database, this should not happen!")]
	TooManyInstances,
	#[error("missing instances")]
	MissingInstance,
	#[error("your library version can't be automatically updated, please recreate your library")]
	CriticalUpdateError,

	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	// #[error(transparent)]
	// VersionManager(#[from] VersionManagerError<LibraryConfigVersion>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
