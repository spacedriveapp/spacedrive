use std::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use futures::ready;
use pin_project_lite::pin_project;
use tokio::io::AsyncBufRead;

pin_project! {
	/// Count the lines of an [AsyncBufRead] without reading the whole file into memory.
	/// We load a chunk, check for `\n` chars, count them and repeat until the file is read.
	///
	/// The implementation has been copied heavily from the similar [tokio::io::Lines].
	pub struct CountLines<B: AsyncBufRead> {
		#[pin]
		reader: B,
		lines: usize,
	}
}

impl<B: AsyncBufRead> CountLines<B> {
	/// Create a new [CountLines] from an [AsyncBufRead]
	pub fn new(reader: B) -> Self {
		Self { reader, lines: 0 }
	}
}

impl<B: AsyncBufRead> Future for CountLines<B> {
	type Output = Result<usize, std::io::Error>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let mut this = self.project();
		loop {
			let i = match ready!(this.reader.as_mut().poll_fill_buf(cx)) {
				Ok(buf) if buf.len() == 0 => return Poll::Ready(Ok(*this.lines)),
				Ok(buf) => match memchr::memchr(b'\n', buf) {
					Some(i) => {
						*this.lines += 1;

						i + 1 // consume up to `\n` & +1 for the newline char
					}
					None => buf.len(), // no newline found, consume whole buffer
				},
				Err(e) => return Poll::Ready(Err(e)),
			};

			this.reader.as_mut().consume(i);
		}
	}
}
