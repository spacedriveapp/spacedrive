use tokio::io::{AsyncRead, AsyncReadExt};

// will probs have more variants in future
#[derive(Debug, PartialEq, Eq)]
pub enum SyncMessage {
	NewOperations,
}

impl SyncMessage {
	// TODO: Per field errors for better error handling
	// TODO: Using `decode::Error` instead of `io::Result`
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
	use super::*;

	#[tokio::test]
	async fn test_types() {
		{
			let original = SyncMessage::NewOperations;

			let mut cursor = std::io::Cursor::new(original.to_bytes());
			let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			assert_eq!(original, result);
		}
	}
}
