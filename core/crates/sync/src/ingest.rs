use sd_p2p::{spacetunnel::Tunnel, PeerId};
use sd_sync::CRDTOperation;
use tokio::sync::mpsc;
use uhlc::NTP64;
use uuid::Uuid;

use crate::Timestamps;

pub struct Actor {
	pub events: mpsc::Sender<Event>,
}

#[must_use]
pub enum Request {
	Messages {
		tunnel: Tunnel,
		timestamps: Vec<(Uuid, NTP64)>,
	},
	Ingest(Vec<CRDTOperation>),
}

#[derive(Debug)]
pub enum Event {
	Notification(NotificationEvent),
	Messages(MessagesEvent),
}

#[derive(Debug)]
pub struct MessagesEvent {
	pub instance_id: Uuid,
	pub messages: Vec<CRDTOperation>,
}

#[derive(Debug)]
pub struct NotificationEvent {
	pub tunnel: Tunnel,
}

#[derive(Debug)]
pub enum State {
	WaitingForNotification,
	ExecutingMessagesRequest,
	Ingesting,
}

#[macro_export]
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
	pub fn spawn(timestamps: Timestamps) -> (Self, mpsc::Receiver<Request>) {
		let (req_tx, req_rx) = mpsc::channel(4);
		let (events_tx, mut events_rx) = mpsc::channel(4);

		tokio::spawn(async move {
			let mut state = State::WaitingForNotification;

			loop {
				dbg!(&state);

				state = match state {
					State::WaitingForNotification => {
						let notification = wait!(events_rx, Event::Notification(n) => n);

						// req_tx.send(Request::Messages(tunnel, peer_id, 69));

						// let notification = wait!(
						// 	events_rx,
						// 	Incoming::Notification(notification) => notification
						// );

						req_tx
							.send(Request::Messages {
								tunnel: notification.tunnel,
								timestamps: timestamps
									.read()
									.await
									.iter()
									.map(|(&k, &v)| (k, v))
									.collect(),
							})
							.await
							.ok();

						State::ExecutingMessagesRequest
					}
					State::ExecutingMessagesRequest => {
						let event = wait!(events_rx, Event::Messages(event) => event);

						req_tx
							.send(Request::Ingest(event.messages.clone()))
							.await
							.ok();

						dbg!(&event.messages);

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

	pub async fn notify(&self, tunnel: Tunnel, _peer_id: PeerId) {
		self.events
			.send(Event::Notification(NotificationEvent { tunnel }))
			.await
			.ok();
	}
}

// #[must_use]
// pub struct ReqRes<TReq, TResp> {
// 	request: TReq,
// 	response_sender: oneshot::Sender<TResp>,
// }

// impl<TReq, TResp> ReqRes<TReq, TResp> {
// 	pub async fn send<TContainer>(
// 		request: TReq,
// 		container_fn: impl Fn(Self) -> TContainer,
// 		sender: &mpsc::Sender<TContainer>,
// 	) -> TResp {
// 		let (tx, rx) = oneshot::channel();

// 		let payload = container_fn(Self {
// 			request,
// 			response_sender: tx,
// 		});

// 		sender.send(payload).await.ok();

// 		rx.await.unwrap()
// 	}

// 	#[must_use]
// 	pub fn split(self) -> (TReq, impl FnOnce(TResp)) {
// 		(self.request, |response| {
// 			self.response_sender.send(response).ok();
// 		})
// 	}

// 	pub async fn map<
// 		TFn: FnOnce(TReq) -> TFut,
// 		TFut: Future<Output = Result<TResp, TErr>>,
// 		TErr,
// 	>(
// 		self,
// 		func: TFn,
// 	) -> Result<(), TErr> {
// 		self.response_sender.send(func(self.request).await?).ok();
// 		Ok(())
// 	}
// }
