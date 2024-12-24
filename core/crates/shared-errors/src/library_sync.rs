use tokio::task::JoinError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("serialization error: {0}")]
	Serialization(#[from] rmp_serde::encode::Error),
	#[error("deserialization error: {0}")]
	Deserialization(#[from] rmp_serde::decode::Error),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("PrismaSync error: {0}")]
	PrismaSync(#[from] sd_prisma::prisma_sync::Error),
	#[error("invalid model id: {0}")]
	InvalidModelId(sd_sync::ModelId),
	#[error("tried to write an empty operations list")]
	EmptyOperations,
	#[error("device not found: {0}")]
	DeviceNotFound(sd_sync::DevicePubId),
	#[error("processes crdt task panicked")]
	ProcessCrdtPanic(JoinError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::Database(e) => e.into(),
			Error::InvalidModelId(id) => Self::new(
				rspc::ErrorCode::BadRequest,
				format!("Invalid model id <id={id}>"),
			),
			_ => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Internal sync error".to_string(),
				e,
			),
		}
	}
}
