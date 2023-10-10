use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use axum::http::HeaderMap;
use bytes::Bytes;
use futures::Stream;
use http_body::Body;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncReadExt, Take};
use tokio_util::io::ReaderStream;

// This code was taken from: https://github.com/tower-rs/tower-http/blob/e8eb54966604ea7fa574a2a25e55232f5cfe675b/tower-http/src/services/fs/mod.rs#L30
pin_project! {
	// NOTE: This could potentially be upstreamed to `http-body`.
	/// Adapter that turns an [`impl AsyncRead`][tokio::io::AsyncRead] to an [`impl Body`][http_body::Body].
	#[derive(Debug)]
	pub struct AsyncReadBody<T> {
		#[pin]
		reader: ReaderStream<T>,
	}
}

impl<T> AsyncReadBody<T>
where
	T: AsyncRead,
{
	pub(crate) fn with_capacity_limited(
		read: T,
		capacity: usize,
		max_read_bytes: u64,
	) -> AsyncReadBody<Take<T>> {
		AsyncReadBody {
			reader: ReaderStream::with_capacity(read.take(max_read_bytes), capacity),
		}
	}
}

impl<T> Body for AsyncReadBody<T>
where
	T: AsyncRead,
{
	type Data = Bytes;
	type Error = io::Error;

	fn poll_data(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<Self::Data, Self::Error>>> {
		self.project().reader.poll_next(cx)
	}

	fn poll_trailers(
		self: Pin<&mut Self>,
		_cx: &mut Context<'_>,
	) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
		Poll::Ready(Ok(None))
	}
}
