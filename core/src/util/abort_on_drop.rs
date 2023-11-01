use futures::{pin_mut, Future, Stream};

pub struct AbortOnDrop<T>(pub tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
	fn drop(&mut self) {
		self.0.abort()
	}
}

impl<T> Future for AbortOnDrop<T> {
	type Output = Result<T, tokio::task::JoinError>;

	fn poll(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Self::Output> {
		let handle = &mut self.0;

		pin_mut!(handle);

		handle.poll(cx)
	}
}

impl<T> Stream for AbortOnDrop<T> {
	type Item = Result<(), rspc::Error>; // TODO: Use `rspc::Infallible` -> Right now inner and outer result must match in rspc which is cringe???

	fn poll_next(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Self::Item>> {
		let handle = &mut self.0;

		pin_mut!(handle);

		handle.poll(cx).map(|_| None)
	}
}
