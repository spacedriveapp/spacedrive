use tokio::sync::mpsc;

pub trait ActorTypes {
	type Event;
	type Request;
	type Handler;
}

pub struct ActorIO<T: ActorTypes> {
	pub event_rx: mpsc::Receiver<T::Event>,
	pub req_tx: mpsc::Sender<T::Request>,
}

impl<T: ActorTypes> ActorIO<T> {
	pub async fn send(&self, value: T::Request) -> Result<(), mpsc::error::SendError<T::Request>> {
		self.req_tx.send(value).await
	}
}

pub struct HandlerIO<T: ActorTypes> {
	pub event_tx: mpsc::Sender<T::Event>,
	pub req_rx: mpsc::Receiver<T::Request>,
}

pub fn create_actor_io<T: ActorTypes>() -> (ActorIO<T>, HandlerIO<T>) {
	let (req_tx, req_rx) = mpsc::channel(20);
	let (event_tx, event_rx) = mpsc::channel(20);

	(ActorIO { event_rx, req_tx }, HandlerIO { event_tx, req_rx })
}

#[macro_export]
macro_rules! wait {
	($rx:expr, $pattern:pat $(=> $expr:expr)?) => {
		loop {
			match $rx.recv().await {
				Some($pattern) => break $($expr)?,
				_ => continue
			}
		}
	};
}
