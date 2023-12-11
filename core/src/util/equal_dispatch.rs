use tokio::sync::mpsc;

pub struct EqualDispatch<T> {
	// Last channel we sent on
	last: usize,
	// Channels to send on
	channels: Vec<mpsc::Sender<T>>,
}

impl<T> EqualDispatch<T> {
	pub fn new(channels: Vec<mpsc::Sender<T>>) -> Self {
		Self { last: 0, channels }
	}

	pub async fn send(&mut self, mut item: T) -> Result<(), ()> {
		let mut last = self.last;
		let mut send_attempts = 0;
		loop {
			if send_attempts > 5 {
				return Err(());
			}

			last = (last + 1) % self.channels.len();
			match self.channels[last].send(item).await {
				Ok(_) => break,
				Err(err) => item = err.0,
			}
			send_attempts += 1;
		}
		self.last = last;
		Ok(())
	}
}
