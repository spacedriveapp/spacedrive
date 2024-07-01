use async_channel as chan;

pub trait ActorTypes {
	type Event: Send;
	type Request: Send;
	type Handler;
}

pub struct ActorIO<T: ActorTypes> {
	pub event_rx: chan::Receiver<T::Event>,
	pub req_tx: chan::Sender<T::Request>,
}

impl<T: ActorTypes> Clone for ActorIO<T> {
	fn clone(&self) -> Self {
		Self {
			event_rx: self.event_rx.clone(),
			req_tx: self.req_tx.clone(),
		}
	}
}

impl<T: ActorTypes> ActorIO<T> {
	pub async fn send(&self, value: T::Request) -> Result<(), chan::SendError<T::Request>> {
		self.req_tx.send(value).await
	}
}

pub struct HandlerIO<T: ActorTypes> {
	pub event_tx: chan::Sender<T::Event>,
	pub req_rx: chan::Receiver<T::Request>,
}

pub fn create_actor_io<T: ActorTypes>() -> (ActorIO<T>, HandlerIO<T>) {
	let (req_tx, req_rx) = chan::bounded(32);
	let (event_tx, event_rx) = chan::bounded(32);

	(ActorIO { event_rx, req_tx }, HandlerIO { event_tx, req_rx })
}
