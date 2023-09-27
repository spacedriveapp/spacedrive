use std::{
	io::{self, ErrorKind},
	pin::Pin,
	task::{Context, Poll},
};

use libp2p::{futures::AsyncWriteExt, Stream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt as TokioAsyncWriteExt, ReadBuf};
use tokio_util::compat::Compat;

pub const BROADCAST_DISCRIMINATOR: u8 = 0;
pub const UNICAST_DISCRIMINATOR: u8 = 1;

/// A broadcast is a message sent to many peers in the network.
/// Due to this it is not possible to respond to a broadcast.
#[derive(Debug)]
pub struct BroadcastStream(Option<Compat<Stream>>);

impl BroadcastStream {
	pub(crate) fn new(stream: Compat<Stream>) -> Self {
		Self(Some(stream))
	}

	async fn close_inner(mut io: Compat<Stream>) -> Result<(), io::Error> {
		io.write_all(&[b'D']).await?;
		io.flush().await?;

		match io.into_inner().close().await {
			Ok(_) => Ok(()),
			Err(err) if err.kind() == ErrorKind::ConnectionReset => Ok(()), // The other end shut the connection before us
			Err(err) => Err(err),
		}
	}
}

impl AsyncRead for BroadcastStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0.as_mut().expect("'BroadcastStream' can only be 'None' if this method is called after 'Drop' which ain't happening!")).poll_read(cx, buf)
	}
}

impl Drop for BroadcastStream {
	fn drop(&mut self) {
		// This may be `None` if the user manually called `Self::close`
		if let Some(stream) = self.0.take() {
			tokio::spawn(async move {
				Self::close_inner(stream).await.unwrap();
			});
		}
	}
}

/// A unicast stream is a direct stream to a specific peer.
#[derive(Debug)]
pub struct UnicastStream(Compat<Stream>);

// TODO: Utils for sending msgpack and stuff over the stream. -> Have a max size of reading buffers so we are less susceptible to DoS attacks.

impl UnicastStream {
	pub(crate) fn new(io: Compat<Stream>) -> Self {
		Self(io)
	}

	pub(crate) async fn write_discriminator(&mut self) -> io::Result<()> {
		// TODO: Timeout if the peer doesn't accept the byte quick enough
		self.0.write_all(&[UNICAST_DISCRIMINATOR]).await
	}

	pub async fn close(self) -> Result<(), io::Error> {
		self.0.into_inner().close().await
	}
}

impl AsyncRead for UnicastStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
	}
}

impl AsyncWrite for UnicastStream {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
	}
}
