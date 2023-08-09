use sd_core_sync::GetOpsArgs;
use sd_p2p::proto::{decode, encode};
use sd_sync::CRDTOperation;
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, PartialEq, Eq)]
pub enum SyncMessage {
	NewOperations,
	OperationsRequest(GetOpsArgs),
	OperationsRequestResponse(Vec<CRDTOperation>),
}

impl SyncMessage {
	// TODO: Per field errors for better error handling
	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			b'N' => Ok(Self::NewOperations),
			b'R' => Ok(Self::OperationsRequest(
				rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
			)),
			b'P' => Ok(Self::OperationsRequestResponse(
				// TODO: Error handling
				rmp_serde::from_slice(&decode::buf(stream).await.unwrap()).unwrap(),
			)),
			header => Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Invalid sync message header: {}", (header as char)),
			)),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::NewOperations => vec![b'N'],
			Self::OperationsRequest(args) => {
				let mut buf = vec![b'R'];

				// TODO: Error handling
				encode::buf(&mut buf, &rmp_serde::to_vec_named(&args).unwrap());
				buf
			}
			Self::OperationsRequestResponse(ops) => {
				let mut buf = vec![b'P'];

				// TODO: Error handling
				encode::buf(&mut buf, &rmp_serde::to_vec_named(&ops).unwrap());
				buf
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use sd_core_sync::NTP64;
	use sd_sync::SharedOperation;
	use serde_json::Value;
	use uuid::Uuid;

	use super::*;

	#[tokio::test]
	async fn test_types() {
		{
			let original = SyncMessage::NewOperations;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncMessage::OperationsRequest(GetOpsArgs {
				clocks: vec![],
				count: 0,
			});

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncMessage::OperationsRequestResponse(vec![]);

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncMessage::OperationsRequestResponse(vec![CRDTOperation {
				instance: Uuid::new_v4(),
				timestamp: NTP64(0),
				id: Uuid::new_v4(),
				typ: sd_sync::CRDTOperationType::Shared(SharedOperation {
					record_id: Value::Null,
					model: "name".to_string(),
					data: sd_sync::SharedOperationData::Create,
				}),
			}]);

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}
	}
}
