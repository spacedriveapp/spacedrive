use std::{
	fmt, io,
	pin::Pin,
	task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

use crate::RemoteIdentity;

trait IoStream: AsyncRead + AsyncWrite {}
impl<S: AsyncRead + AsyncWrite> IoStream for S {}

/// A unicast stream is a direct stream to a specific peer.
pub struct UnicastStream {
	io: Pin<Box<dyn IoStream + Send>>,
	remote: RemoteIdentity,
}

impl fmt::Debug for UnicastStream {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("UnicastStream")
			.field("remote", &self.remote)
			.finish()
	}
}

impl UnicastStream {
	pub fn new<S: AsyncRead + AsyncWrite + Send + 'static>(remote: RemoteIdentity, io: S) -> Self {
		Self {
			io: Box::pin(io),
			remote,
		}
	}

	#[must_use]
	pub fn remote_identity(&self) -> RemoteIdentity {
		self.remote
	}

	pub async fn close(mut self) -> Result<(), io::Error> {
		self.io.shutdown().await
	}
}

impl AsyncRead for UnicastStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_read(cx, buf)
	}
}

impl AsyncWrite for UnicastStream {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
	}
}
