use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, PartialEq, Eq)]
pub enum SyncMessage {
	NewOperations,
	OperationsRequest(u8),
	OperationsRequestResponse(u8),
}

impl SyncMessage {
	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			b'N' => Ok(Self::NewOperations),
			b'R' => Ok(Self::OperationsRequest(stream.read_u8().await?)),
			b'P' => Ok(Self::OperationsRequestResponse(stream.read_u8().await?)),
			header => Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!(
					"Invalid sync message header: {}",
					(header as char).to_string()
				),
			)),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::NewOperations => vec![b'N'],
			Self::OperationsRequest(s) => vec![b'R', *s],
			Self::OperationsRequestResponse(s) => vec![b'P', *s],
		}
	}
}

#[cfg(test)]
mod tests {
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
			let original = SyncMessage::OperationsRequest(1);

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}

		{
			let original = SyncMessage::OperationsRequestResponse(2);

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}
	}
}
