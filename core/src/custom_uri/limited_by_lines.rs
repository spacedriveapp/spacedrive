use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use axum::http::HeaderMap;
use bytes::{BufMut, Bytes, BytesMut};
use futures::ready;
use http_body::Body;
use pin_project_lite::pin_project;
use tokio::io::AsyncBufRead;

pin_project! {
	// Serve a file but limited by a certain number of lines (`\n` we ain't a CLRF kinda place)
	// Be aware a single line could be massive and this will not limit that but as the file is streamed from disk it shouldn't blow up the memory usage or anything.
	//
	// Be aware this will also send a cursor at the end of the body but that could be made optional if this is needed somewhere else.
	#[derive(Debug)]
	pub struct LimitedByLinesBody<T> {
		#[pin]
		reader: T,
		// Counts down from the intended number of lines to zero.
		// When this hits zero the stream will end.
		lines: u64,
	}
}

impl<T> LimitedByLinesBody<T>
where
	T: AsyncBufRead,
{
	pub(crate) fn with_lines_limited(reader: T, lines: u64) -> LimitedByLinesBody<T> {
		LimitedByLinesBody { reader, lines }
	}
}

impl<T> Body for LimitedByLinesBody<T>
where
	T: AsyncBufRead,
{
	type Data = Bytes;
	type Error = io::Error;

	fn poll_data(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<Self::Data, Self::Error>>> {
		let mut this = self.project();

		if *this.lines == 0 {
			return Poll::Ready(None);
		}

		match ready!(this.reader.as_mut().poll_fill_buf(cx)) {
			Ok(buf) if buf.len() == 0 => return Poll::Ready(None),
			Ok(buf) => {
				let i = match memchr::memchr(b'\n', buf) {
					Some(i) => {
						*this.lines -= 1; // We are counting from total to zero

						i + 1 // consume up to `\n` & +1 for the newline char // TODO: Is this gonna cause us to skip sending a byte to the frontend
					}
					None => buf.len(), // no newline found, consume whole buffer
				};

				let end = if *this.lines == 0 {
					// If we are on the last line then we need to trim the trailing newline
					i - 1
				} else {
					i
				};

				let mut bytes = BytesMut::with_capacity(buf.len());
				bytes.extend_from_slice(&buf[..end]);
				this.reader.consume(i);

				if *this.lines == 0 {
					// bytes.extend_from_slice(&[b'\n', b'\n']);
					// bytes.put(); // TODO: Cursor
				}

				Poll::Ready(Some(Ok(bytes.freeze())))
			}
			Err(e) => Poll::Ready(Some(Err(e))), // TODO: will this poll for `None` next??? I think it will
		}
	}

	fn poll_trailers(
		self: Pin<&mut Self>,
		_cx: &mut Context<'_>,
	) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
		Poll::Ready(Ok(None))
	}
}
