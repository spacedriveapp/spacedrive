use std::{
	fmt,
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use flume::r#async::RecvFut;
use libp2p::futures::FutureExt;

/// A future that polls multiple flume channels.
pub(crate) struct MultiFlume<T: 'static>(Vec<RecvFut<'static, T>>);

impl<T: 'static> fmt::Debug for MultiFlume<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("MultiFlume")
			.field("len", &self.0.len())
			.finish()
	}
}

impl<T: 'static> Default for MultiFlume<T> {
	fn default() -> Self {
		Self(Vec::new())
	}
}

impl<T: 'static> MultiFlume<T> {
	pub fn push(&mut self, recv: RecvFut<'static, T>) {
		self.0.push(recv);
	}
}

impl<T: 'static> Future for MultiFlume<T> {
	type Output = Result<T, flume::RecvError>;

	// A quick aside on future stravation. Futures earlier in the vec will be prioritise, and could starve later futures.
	// For now this is fine but if it becomes a problem in practice we can randomise the order of the vector each poll.
	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let mut is_pending = false;
		while !is_pending {
			for recv in self.0.iter_mut() {
				match recv.poll_unpin(cx) {
					Poll::Ready(event) => return Poll::Ready(event),
					Poll::Pending => is_pending = true,
				}
			}
		}

		Poll::Pending
	}
}
