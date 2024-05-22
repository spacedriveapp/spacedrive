use std::{
	pin::Pin,
	task::{Context, Poll},
};

use bytes::Bytes;
use tokio::io::{self, AsyncWrite};
use tokio_util::sync::PollSender;

/// Allowing wrapping an `mpsc::Sender` into an `AsyncWrite`
pub struct MpscToAsyncWrite(PollSender<io::Result<Bytes>>);

impl MpscToAsyncWrite {
	#[allow(dead_code)]
	pub fn new(sender: PollSender<io::Result<Bytes>>) -> Self {
		Self(sender)
	}
}

impl AsyncWrite for MpscToAsyncWrite {
	fn poll_write(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<Result<usize, io::Error>> {
		#[allow(clippy::unwrap_used)]
		match self.0.poll_reserve(cx) {
			Poll::Ready(Ok(())) => {
				self.0.send_item(Ok(Bytes::from(buf.to_vec()))).unwrap();
				Poll::Ready(Ok(buf.len()))
			}
			Poll::Ready(Err(_)) => todo!(),
			Poll::Pending => Poll::Pending,
		}
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		Poll::Ready(Ok(()))
	}

	fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
		Poll::Ready(Ok(()))
	}
}
