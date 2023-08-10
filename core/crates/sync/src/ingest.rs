use std::{ops::Deref, sync::Arc};

use sd_p2p::spacetunnel::Tunnel;
use sd_prisma::{prisma::*, prisma_sync::ModelSyncData};
use sd_sync::*;
use sd_utils::uuid_to_bytes;
use serde_json::to_vec;
use tokio::sync::mpsc;
use uhlc::{Timestamp, NTP64};
use uuid::Uuid;

use crate::{actor::*, wait, SharedState};

#[must_use]
/// Stuff that can be handled outside the actor
pub enum Request {
	Messages {
		tunnel: Tunnel,
		timestamps: Vec<(Uuid, NTP64)>,
	},
	Ingested,
}

/// Stuff that the actor consumes
#[derive(Debug)]
pub enum Event {
	Notification(NotificationEvent),
	Messages(MessagesEvent),
}

#[derive(Debug, Default)]
pub enum State {
	#[default]
	WaitingForNotification,
	RetrievingMessages(Tunnel),
	Ingesting(MessagesEvent),
}

pub struct Actor {
	state: Option<State>,
	shared: Arc<SharedState>,
	io: ActorIO<Self>,
}

impl Actor {
	async fn tick(mut self) -> Option<Self> {
		let state = match self.state.take()? {
			State::WaitingForNotification => {
				let notification = wait!(self.io.event_rx, Event::Notification(n) => n);

				State::RetrievingMessages(notification.tunnel)
			}
			State::RetrievingMessages(tunnel) => {
				self.io
					.send(Request::Messages {
						tunnel,
						timestamps: self
							.timestamps
							.read()
							.await
							.iter()
							.map(|(&k, &v)| (k, v))
							.collect(),
					})
					.await
					.ok();

				State::Ingesting(wait!(self.io.event_rx, Event::Messages(event) => event))
			}
			State::Ingesting(event) => {
				let count = event.messages.len();

				dbg!(&event.messages);

				for op in event.messages {
					let fut = self.receive_crdt_operation(op);
					fut.await;
				}

				println!("Ingested {count} messages!");

				match event.has_more {
					true => State::RetrievingMessages(event.tunnel),
					false => State::WaitingForNotification,
				}
			}
		};

		Some(Self {
			state: Some(state),
			..self
		})
	}

	pub fn spawn(shared: Arc<SharedState>) -> SplitHandlerIO<Self> {
		let (actor_io, handler_io) = create_actor_io::<Self>(|event_tx| Handler { event_tx });

		tokio::spawn(async move {
			let mut this = Self {
				state: Some(Default::default()),
				io: actor_io,
				shared,
			};

			loop {
				this = match this.tick().await {
					Some(this) => this,
					None => break,
				};
			}
		});

		handler_io.split()
	}

	async fn receive_crdt_operation(&mut self, op: CRDTOperation) {
		self.clock
			.update_with_timestamp(&Timestamp::new(op.timestamp, op.instance.into()))
			.ok();

		let mut timestamp = {
			let mut clocks = self.timestamps.write().await;
			clocks
				.entry(op.instance)
				.or_insert_with(|| op.timestamp)
				.clone()
		};

		if timestamp < op.timestamp {
			timestamp = op.timestamp;
		}

		let op_instance = op.instance;

		let is_old = self.compare_message(&op).await;

		if !is_old {
			self.apply_op(op).await.ok();
		}

		self.db
			._transaction()
			.run({
				let timestamps = self.timestamps.clone();
				|db| async move {
					match db
						.instance()
						.update(
							instance::pub_id::equals(uuid_to_bytes(op_instance)),
							vec![instance::timestamp::set(Some(timestamp.as_u64() as i64))],
						)
						.exec()
						.await
					{
						Ok(_) => {
							timestamps.write().await.insert(op_instance, timestamp);
							Ok(())
						}
						Err(e) => Err(e),
					}
				}
			})
			.await
			.unwrap();
	}

	async fn apply_op(&mut self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		ModelSyncData::from_op(op.typ.clone())
			.unwrap()
			.exec(&self.db)
			.await?;

		match &op.typ {
			CRDTOperationType::Shared(shared_op) => {
				shared_op_db(&op, shared_op)
					.to_query(&self.db)
					.exec()
					.await?;
			}
			CRDTOperationType::Relation(relation_op) => {
				relation_op_db(&op, relation_op)
					.to_query(&self.db)
					.exec()
					.await?;
			}
		}

		self.io.req_tx.send(Request::Ingested).await.ok();

		Ok(())
	}

	async fn compare_message(&mut self, op: &CRDTOperation) -> bool {
		let old_timestamp = match &op.typ {
			CRDTOperationType::Shared(shared_op) => {
				let newer_op = self
					.db
					.shared_operation()
					.find_first(vec![
						shared_operation::timestamp::gte(op.timestamp.as_u64() as i64),
						shared_operation::model::equals(shared_op.model.to_string()),
						shared_operation::record_id::equals(
							serde_json::to_vec(&shared_op.record_id).unwrap(),
						),
						shared_operation::kind::equals(shared_op.kind().to_string()),
					])
					.order_by(shared_operation::timestamp::order(SortOrder::Desc))
					.exec()
					.await
					.unwrap();

				newer_op.map(|newer_op| newer_op.timestamp)
			}
			CRDTOperationType::Relation(relation_op) => {
				let newer_op = self
					.db
					.relation_operation()
					.find_first(vec![
						relation_operation::timestamp::gte(op.timestamp.as_u64() as i64),
						relation_operation::relation::equals(relation_op.relation.to_string()),
						relation_operation::item_id::equals(
							serde_json::to_vec(&relation_op.relation_item).unwrap(),
						),
						relation_operation::kind::equals(relation_op.kind().to_string()),
					])
					.order_by(relation_operation::timestamp::order(SortOrder::Desc))
					.exec()
					.await
					.unwrap();

				newer_op.map(|newer_op| newer_op.timestamp)
			}
		};

		old_timestamp
			.map(|old| old != op.timestamp.as_u64() as i64)
			.unwrap_or_default()
	}
}

impl Deref for Actor {
	type Target = SharedState;

	fn deref(&self) -> &Self::Target {
		&self.shared
	}
}

pub struct Handler {
	pub event_tx: mpsc::Sender<Event>,
}

#[derive(Debug)]
pub struct MessagesEvent {
	pub instance_id: Uuid,
	pub messages: Vec<CRDTOperation>,
	pub has_more: bool,
	pub tunnel: Tunnel,
}

#[derive(Debug)]
pub struct NotificationEvent {
	pub tunnel: Tunnel,
}

impl ActorTypes for Actor {
	type Event = Event;
	type Request = Request;
	type Handler = Handler;
}

fn shared_op_db(op: &CRDTOperation, shared_op: &SharedOperation) -> shared_operation::Create {
	shared_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: shared_op.kind().to_string(),
		data: to_vec(&shared_op.data).unwrap(),
		model: shared_op.model.to_string(),
		record_id: to_vec(&shared_op.record_id).unwrap(),
		_params: vec![],
	}
}

fn relation_op_db(
	op: &CRDTOperation,
	relation_op: &RelationOperation,
) -> relation_operation::Create {
	relation_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: relation_op.kind().to_string(),
		data: to_vec(&relation_op.data).unwrap(),
		relation: relation_op.relation.to_string(),
		item_id: to_vec(&relation_op.relation_item).unwrap(),
		group_id: to_vec(&relation_op.relation_group).unwrap(),
		_params: vec![],
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
