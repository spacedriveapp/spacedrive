use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use sd_p2p::UnicastStream;

#[derive(Debug)]
pub struct Tunnel {
	stream: UnicastStream,
}

impl Tunnel {
	// TODO: Proper errors
	pub async fn initiator(mut stream: UnicastStream) -> Result<Self, &'static str> {
		stream
			.write_all(&[b'T'])
			.await
			.map_err(|_| "Error writing discriminator")?;

		// TODO: Do pairing + authentication

		Ok(Self { stream })
	}

	// TODO: Proper errors
	pub async fn responder(mut stream: UnicastStream) -> Result<Self, &'static str> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(|_| "Error reading discriminator. Is this stream actually a tunnel?")?;
		if discriminator != b'T' {
			return Err("Invalid discriminator. Is this stream actually a tunnel?");
		}

		// TODO: Do pairing + authentication

		Ok(Self { stream })
	}
}

impl AsyncRead for Tunnel {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		// TODO: Do decryption

		Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
	}
}

impl AsyncWrite for Tunnel {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		// TODO: Do encryption

		Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().stream).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
	}
}

// TODO: Unit tests
