use tokio::io::{AsyncRead, AsyncReadExt};

// will probs have more variants in future
#[derive(Debug, PartialEq, Eq)]
pub enum SyncMessage {
	NewOperations,
}

impl SyncMessage {
	// TODO: Per field errors for better error handling
	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			b'N' => Ok(Self::NewOperations),
			header => Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Invalid sync message header: {}", (header as char)),
			)),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::NewOperations => vec![b'N'],
		}
	}
}

#[cfg(test)]
mod tests {
	// use sd_core_sync::NTP64;
	// use sd_sync::SharedOperation;
	// use serde_json::Value;
	// use uuid::Uuid;

	use super::*;

	#[tokio::test]
	async fn test_types() {
		{
			let original = SyncMessage::NewOperations;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		// {
		// 	let original = SyncMessage::OperationsRequest(GetOpsArgs {
		// 		clocks: vec![],
		// 		count: 0,
		// 	});

		// 	let mut cursor = std::io::Cursor::new(original.to_bytes());
		// 	let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
		// 	assert_eq!(original, result);
		// }

		// {
		// 	let original = SyncMessage::OperationsRequestResponse(vec![]);

		// 	let mut cursor = std::io::Cursor::new(original.to_bytes());
		// 	let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
		// 	assert_eq!(original, result);
		// }

		// {
		// 	let original = SyncMessage::OperationsRequestResponse(vec![CRDTOperation {
		// 		instance: Uuid::new_v4(),
		// 		timestamp: NTP64(0),
		// 		id: Uuid::new_v4(),
		// 		typ: sd_sync::CRDTOperationType::Shared(SharedOperation {
		// 			record_id: Value::Null,
		// 			model: "name".to_string(),
		// 			data: sd_sync::SharedOperationData::Create,
		// 		}),
		// 	}]);

		// 	let mut cursor = std::io::Cursor::new(original.to_bytes());
		// 	let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
		// 	assert_eq!(original, result);
		// }
	}
}
