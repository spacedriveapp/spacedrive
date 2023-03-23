use std::{
	io::{self, ErrorKind},
	pin::Pin,
	task::{Context, Poll},
};

use libp2p::{futures::AsyncWriteExt, swarm::NegotiatedSubstream};
use tokio::io::{
	AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt as TokioAsyncWriteExt, ReadBuf,
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};
use tracing::error;

pub const BROADCAST_DISCRIMINATOR: u8 = 0;
pub const UNICAST_DISCRIMINATOR: u8 = 1;

#[derive(Debug)]
pub enum SpaceTimeStream {
	Broadcast(BroadcastStream),
	Unicast(UnicastStream),
}

impl SpaceTimeStream {
	pub(crate) async fn from_stream(io: NegotiatedSubstream) -> Self {
		let mut io = io.compat();
		let discriminator = io.read_u8().await.unwrap(); // TODO: Timeout on this
		match discriminator {
			BROADCAST_DISCRIMINATOR => Self::Broadcast(BroadcastStream(Some(io))),
			UNICAST_DISCRIMINATOR => Self::Unicast(UnicastStream(io)),
			_ => todo!(), // TODO: Error handling
		}
	}

	pub fn stream_type(&self) -> &'static str {
		match self {
			Self::Broadcast(_) => "broadcast",
			Self::Unicast(_) => "unicast",
		}
	}

	pub async fn close(self) -> Result<(), io::Error> {
		match self {
			Self::Broadcast(mut stream) => {
				if let Some(stream) = stream.0.take() {
					BroadcastStream::close_inner(stream).await
				} else if cfg!(debug_assertions) {
					panic!("'BroadcastStream' should never be 'None' here!");
				} else {
					error!("'BroadcastStream' should never be 'None' here!");
					Ok(())
				}
			}
			Self::Unicast(stream) => stream.0.into_inner().close().await,
		}
	}
}

impl AsyncRead for SpaceTimeStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		match self.get_mut() {
			Self::Broadcast(stream) => Pin::new(stream).poll_read(cx, buf),
			Self::Unicast(stream) => Pin::new(stream).poll_read(cx, buf),
		}
	}
}

/// A broadcast is a message sent to many peers in the network.
/// Due to this it is not possible to respond to a broadcast.
#[derive(Debug)]
pub struct BroadcastStream(Option<Compat<NegotiatedSubstream>>);

impl BroadcastStream {
	async fn close_inner(mut io: Compat<NegotiatedSubstream>) -> Result<(), io::Error> {
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
pub struct UnicastStream(Compat<NegotiatedSubstream>);

// TODO: Utils for sending msgpack and stuff over the stream. -> Have a max size of reading buffers so we are less susceptible to DoS attacks.

impl UnicastStream {
	pub(crate) fn new(io: NegotiatedSubstream) -> Self {
		Self(io.compat())
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
