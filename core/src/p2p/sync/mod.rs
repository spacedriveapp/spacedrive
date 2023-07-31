use sd_p2p::spacetunnel::Tunnel;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use super::Header;

#[derive(Debug)]
// #[repr(u8)]
pub enum SyncMessage {
	NewOperations,
	OperationsRequest(u8),
	OperationsRequestResponse(u8),
}

impl SyncMessage {
	pub fn header(&self) -> u8 {
		match self {
			Self::NewOperations => b'N',
			Self::OperationsRequest(_) => b'R',
			Self::OperationsRequestResponse(_) => b'P',
		}
	}

	pub async fn from_tunnel(stream: &mut Tunnel) -> std::io::Result<Self> {
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

	pub fn to_bytes(self, library_id: Uuid) -> Vec<u8> {
		// Header -> SyncMessage
		let mut bytes = Header::Sync(library_id).to_bytes();

		bytes.push(self.header());

		match self {
			Self::OperationsRequest(s) => bytes.push(s),
			Self::OperationsRequestResponse(s) => bytes.push(s),
			_ => {}
		}

		bytes
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_types() {
		{
			let original = SyncMessage::NewOperations;

			// let mut cursor = std::io::Cursor::new(original.to_bytes());
			// let result = SyncMessage::from_stream(&mut cursor).await.unwrap();
			// assert_eq!(original, result);
		}

		// let msg = SyncMessage::OperationsRequest(1);

		// let msg = SyncMessage::OperationsRequestResponse(2);
	}
}
