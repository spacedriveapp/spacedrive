use tokio::io::AsyncReadExt;

use crate::spacetime::SpaceTimeStream;

pub struct Tunnel {
	stream: SpaceTimeStream,
}

impl Tunnel {
	// TODO: Proper errors
	pub async fn from_stream(mut stream: SpaceTimeStream) -> Result<Self, &'static str> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(|_| "Error reading discriminator. Is this stream actually a tunnel?")?;
		if discriminator != b'T' {
			return Err("Invalid discriminator. Is this stream actually a tunnel?");
		}

		// TODO: Do pairing

		Ok(Self { stream })
	}
}

// TODO: Unit tests
