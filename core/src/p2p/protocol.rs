use tokio::io::AsyncReadExt;
use uuid::Uuid;

use sd_p2p::{spaceblock::SpacedropRequest, spacetime::SpaceTimeStream};

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	Ping,
	Spacedrop(SpacedropRequest),
	Sync(Uuid, u32),
}

impl Header {
	pub async fn from_stream(stream: &mut SpaceTimeStream) -> Result<Self, ()> {
		let discriminator = stream.read_u8().await.map_err(|e| {
			dbg!(e);
		})?; // TODO: Error handling

		match discriminator {
			0 => match stream {
				SpaceTimeStream::Unicast(stream) => Ok(Self::Spacedrop(
					SpacedropRequest::from_stream(stream).await?,
				)),
				_ => todo!(),
			},
			1 => Ok(Self::Ping),
			2 => {
				let mut uuid = [0u8; 16];
				stream.read_exact(&mut uuid).await.map_err(|_| ())?; // TODO: Error handling

				let mut len = [0; 4];
				stream.read_exact(&mut len).await.map_err(|_| ())?; // TODO: Error handling
				let len = u32::from_le_bytes(len);

				Ok(Self::Sync(Uuid::from_slice(&uuid).unwrap(), len)) // TODO: Error handling
			}
			_ => Err(()),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Spacedrop(transfer_request) => {
				let mut bytes = vec![0];
				bytes.extend_from_slice(&transfer_request.to_bytes());
				bytes
			}
			Self::Ping => vec![1],
			Self::Sync(uuid, len) => {
				let mut bytes = vec![2];
				bytes.extend_from_slice(uuid.as_bytes());

				let len_buf = len.to_le_bytes();
				debug_assert_eq!(len_buf.len(), 4);
				bytes.extend_from_slice(&len_buf);

				bytes
			}
		}
	}
}

// TODO: Unit test it because binary protocols are error prone
// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	#[test]
// 	fn test_proto() {
// 		assert_eq!(
// 			Header::from_bytes(&Header::Ping.to_bytes()),
// 			Ok(Header::Ping)
// 		);

// 		assert_eq!(
// 			Header::from_bytes(&Header::Spacedrop.to_bytes()),
// 			Ok(Header::Spacedrop)
// 		);

// 		let uuid = Uuid::new_v4();
// 		assert_eq!(
// 			Header::from_bytes(&Header::Sync(uuid).to_bytes()),
// 			Ok(Header::Sync(uuid))
// 		);
// 	}
// }
