use std::future::Future;

use sd_p2p::{spacetunnel::Tunnel, PeerId};
use sd_sync::CRDTOperation;
use tokio::sync::{mpsc, oneshot};

pub struct Actor {
	pub events: mpsc::Sender<Event>,
}

#[must_use]
pub struct ReqRes<TReq, TResp> {
	request: TReq,
	response_sender: oneshot::Sender<TResp>,
}

impl<TReq, TResp> ReqRes<TReq, TResp> {
	pub async fn send<TContainer>(
		request: TReq,
		container_fn: impl Fn(Self) -> TContainer,
		sender: &mpsc::Sender<TContainer>,
	) -> TResp {
		let (tx, rx) = oneshot::channel();

		let payload = container_fn(Self {
			request,
			response_sender: tx,
		});

		sender.send(payload).await.ok();

		rx.await.unwrap()
	}

	#[must_use]
	pub fn split(self) -> (TReq, impl FnOnce(TResp)) {
		(self.request, |response| {
			self.response_sender.send(response).ok();
		})
	}

	pub async fn map<
		TFn: FnOnce(TReq) -> TFut,
		TFut: Future<Output = Result<TResp, TErr>>,
		TErr,
	>(
		self,
		func: TFn,
	) -> Result<(), TErr> {
		self.response_sender.send(func(self.request).await?).ok();
		Ok(())
	}
}

#[must_use]
pub enum Request {
	Messages(Tunnel, PeerId, u8),
	Ingest(Vec<CRDTOperation>),
}

pub enum Event {
	Notification(Tunnel, PeerId),
	Messages(u8, Vec<CRDTOperation>),
}

#[derive(Debug)]
pub enum State {
	WaitingForNotification,
	ExecutingMessagesRequest,
	Ingesting,
}

macro_rules! assert_state {
	($pattern:pat, $expr:expr) => {
		let $pattern = $expr else { return; };
	};
}

macro_rules! wait {
	($rx:ident, $pattern:pat $(=> $expr:expr)?) => {
		loop {
			match $rx.recv().await {
				Some($pattern) => break $($expr)?,
				_ => continue
			}
		}
	};
}

impl Actor {
	pub fn spawn() -> (Self, mpsc::Receiver<Request>) {
		let (req_tx, req_rx) = mpsc::channel(4);
		let (events_tx, mut events_rx) = mpsc::channel(4);

		tokio::spawn(async move {
			let mut state = State::WaitingForNotification;

			loop {
				dbg!(&state);

				state = match state {
					State::WaitingForNotification => {
						let (tunnel, peer_id) = wait!(events_rx, Event::Notification(tunnel, peer_id) => (tunnel, peer_id));

						req_tx
							.send(Request::Messages(tunnel, peer_id, 69))
							.await
							.ok();

						State::ExecutingMessagesRequest
					}
					State::ExecutingMessagesRequest => {
						let (data, ops) =
							wait!(events_rx, Event::Messages(data, ops) => (data, ops));

						req_tx.send(Request::Ingest(ops)).await.ok();

						dbg!(&data);

						State::Ingesting
					}
					State::Ingesting => {
						println!("Ingested!");

						State::WaitingForNotification
					}
				};
			}
		});

		(Self { events: events_tx }, req_rx)
	}

	pub async fn notify(&self, tunnel: Tunnel, peer_id: PeerId) {
		self.events
			.send(Event::Notification(tunnel, peer_id))
			.await
			.ok();
	}
}
