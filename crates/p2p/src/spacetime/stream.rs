use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use libp2p::{futures::AsyncWriteExt, swarm::NegotiatedSubstream};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};

#[derive(Debug)]
pub struct SpaceTimeStream(Compat<NegotiatedSubstream>);

// TODO: Utils for sending msgpack and stuff over the stream. -> Have a max size of reading buffers so we are less susceptible to DoS attacks.

impl SpaceTimeStream {
	pub fn new(io: NegotiatedSubstream) -> Self {
		Self(io.compat())
	}

	pub async fn close(self) -> Result<(), io::Error> {
		self.0.into_inner().close().await
	}
}

impl AsyncRead for SpaceTimeStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
	}
}

// impl AsyncBufRead for SpaceTimeStream {
//     fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
//         Pin::new(&mut self.get_mut().0).poll_fill_buf(cx)
//     }

//     fn consume(self: Pin<&mut Self>, amt: usize) {
//         Pin::new(&mut self.get_mut().0).consume(amt)
//     }
// }

impl AsyncWrite for SpaceTimeStream {
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
