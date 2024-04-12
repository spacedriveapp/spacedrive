use std::{
	pin::Pin,
	task::{Context, Poll},
};

use futures_util::Future;
use tokio::sync::mpsc;

/// Construct a stream from a Tokio task.
/// Similar to `tokio_stream::stream!` but not a macro for better DX.
pub struct TaskStream<T> {
	task: tokio::task::JoinHandle<()>,
	receiver: mpsc::Receiver<T>,
}

impl<T: Send + 'static> TaskStream<T> {
	pub fn new<F: Future + Send>(task: impl FnOnce(mpsc::Sender<T>) -> F + Send + 'static) -> Self {
		let (tx, rx) = mpsc::channel(256);
		Self {
			task: tokio::spawn(async move {
				task(tx).await;
			}),
			receiver: rx,
		}
	}
}

impl<T> futures_util::Stream for TaskStream<T> {
	type Item = T;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		self.receiver.poll_recv(cx)
	}
}

impl<T> Drop for TaskStream<T> {
	fn drop(&mut self) {
		self.task.abort();
	}
}
