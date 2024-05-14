use std::{
	pin::Pin,
	task::{Context, Poll},
};

use futures::Stream;
use pin_project_lite::pin_project;

// We limit the number of polls to prevent starvation of other tasks.
// This number is chosen arbitrarily but it is set smaller than `FuturesUnordered` or `StreamUnordered`.
const MAX_POLLS: usize = 15;

pin_project! {
	#[project = BatchedStreamProj]
	pub enum BatchedStream<S> where S: Stream {
		Active {
			#[pin]
			stream: S,
			batch: Vec<S::Item>,
		},
		Complete
	}
}

impl<S: Stream> BatchedStream<S> {
	pub fn new(stream: S) -> Self {
		Self::Active {
			stream,
			batch: Vec::with_capacity(MAX_POLLS),
		}
	}
}

impl<S: Stream + Unpin> Stream for BatchedStream<S> {
	type Item = Vec<S::Item>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		match self.as_mut().project() {
			BatchedStreamProj::Active { mut stream, batch } => {
				for _ in 0..MAX_POLLS {
					match stream.as_mut().poll_next(cx) {
						Poll::Ready(Some(item)) => batch.push(item),
						Poll::Ready(None) => {
							if batch.is_empty() {
								return Poll::Ready(None);
							} else {
								let batch = std::mem::take(batch);
								self.as_mut().set(BatchedStream::Complete);
								return Poll::Ready(Some(batch));
							}
						}
						Poll::Pending => break,
					}
				}

				if batch.is_empty() {
					cx.waker().wake_by_ref();
					Poll::Pending
				} else {
					let batch = std::mem::take(batch);
					Poll::Ready(Some(batch))
				}
			}
			BatchedStreamProj::Complete => Poll::Ready(None),
		}
	}
}
