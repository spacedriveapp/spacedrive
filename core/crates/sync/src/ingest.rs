use std::{
	ops::Deref,
	sync::{atomic::Ordering, Arc},
};

use sd_prisma::{
	prisma::{crdt_operation, SortOrder},
	prisma_sync::ModelSyncData,
};
use sd_sync::{CRDTOperation, OperationKind};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::debug;
use uhlc::{Timestamp, NTP64};
use uuid::Uuid;

use crate::{
	actor::{create_actor_io, ActorIO, ActorTypes},
	db_operation::write_crdt_op_to_db,
	wait, SharedState,
};

#[derive(Debug)]
#[must_use]
/// Stuff that can be handled outside the actor
pub enum Request {
	Messages {
		timestamps: Vec<(Uuid, NTP64)>,
		tx: oneshot::Sender<()>,
	},
	Ingested,
	FinishedIngesting,
}

/// Stuff that the actor consumes
#[derive(Debug)]
pub enum Event {
	Notification,
	Messages(MessagesEvent),
}

#[derive(Debug, Default)]
pub enum State {
	#[default]
	WaitingForNotification,
	RetrievingMessages,
	Ingesting(MessagesEvent),
}

/// The single entrypoint for sync operation ingestion.
/// Requests sync operations in a given timestamp range,
/// and attempts to write them to the syn coperations table along with
/// the actual cell that the operation points to.
///
/// If this actor stops running, no sync operations will
/// be applied to the database, independent of whether systems like p2p
/// or cloud are exchanging messages.
pub struct Actor {
	state: Option<State>,
	shared: Arc<SharedState>,
	io: ActorIO<Self>,
}

impl Actor {
	async fn tick(mut self) -> Option<Self> {
		let state = match self.state.take()? {
			State::WaitingForNotification => {
				self.shared.active.store(false, Ordering::Relaxed);
				self.shared.active_notify.notify_waiters();

				wait!(self.io.event_rx, Event::Notification);

				self.shared.active.store(true, Ordering::Relaxed);
				self.shared.active_notify.notify_waiters();

				State::RetrievingMessages
			}
			State::RetrievingMessages => {
				let (tx, mut rx) = oneshot::channel::<()>();

				let timestamps = self
					.timestamps
					.read()
					.await
					.iter()
					.map(|(&k, &v)| (k, v))
					.collect();

				self.io
					.send(Request::Messages { timestamps, tx })
					.await
					.ok();

				loop {
					tokio::select! {
						biased;
						res = self.io.event_rx.recv() => {
							if let Some(Event::Messages(event)) = res { break State::Ingesting(event) }
						}
						res = &mut rx => {
							if res.is_err() {
								debug!("messages request ignored");
								break State::WaitingForNotification
							 }
						},
					}
				}
			}
			State::Ingesting(event) => {
				if !event.messages.is_empty() {
					debug!(
						"ingesting {} operations: {} to {}",
						event.messages.len(),
						event.messages.first().unwrap().timestamp.as_u64(),
						event.messages.last().unwrap().timestamp.as_u64(),
					);

					for op in event.messages {
						self.receive_crdt_operation(op).await;
					}
				}

				match event.has_more {
					true => State::RetrievingMessages,
					false => {
						self.io.send(Request::FinishedIngesting).await.ok();

						State::WaitingForNotification
					}
				}
			}
		};

		Some(Self {
			state: Some(state),
			..self
		})
	}

	pub fn spawn(shared: Arc<SharedState>) -> Handler {
		let (actor_io, handler_io) = create_actor_io::<Self>();

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

		Handler {
			event_tx: handler_io.event_tx,
			req_rx: Arc::new(Mutex::new(handler_io.req_rx)),
		}
	}

	// where the magic happens
	async fn receive_crdt_operation(
		&mut self,
		mut op: CRDTOperation,
	) -> prisma_client_rust::Result<()> {
		let db = &self.db;

		// first, we update the HLC's timestamp with the incoming one.
		// this involves a drift check + sets the last time of the clock
		self.clock
			.update_with_timestamp(&Timestamp::new(op.timestamp, op.instance.into()))
			.expect("timestamp has too much drift!");

		// read the timestamp for the operation's instance, or insert one if it doesn't exist
		let timestamp = self.timestamps.read().await.get(&op.instance).cloned();

		// copy some fields bc rust ownership
		let op_instance = op.instance;
		let op_timestamp = op.timestamp;

		// resolve conflicts
		// this can be outside the transaction as there's only ever one ingester
		match &mut op.data {
			// don't apply Create operations if the record has been deleted
			sd_sync::CRDTOperationData::Create(_) => {
				let delete = db
					.crdt_operation()
					.find_first(vec![
						crdt_operation::model::equals(op.model as i32),
						crdt_operation::record_id::equals(
							rmp_serde::to_vec(&op.record_id).unwrap(),
						),
						crdt_operation::kind::equals(OperationKind::Delete.to_string()),
					])
					.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
					.exec()
					.await?;

				if delete.is_some() {
					return Ok(());
				}
			}
			// don't apply Update operations if the record hasn't been created, or a newer Update for the same field has been applied
			sd_sync::CRDTOperationData::Update { field, .. } => {
				let (create, update) = db
					._batch((
						db.crdt_operation()
							.find_first(vec![
								crdt_operation::model::equals(op.model as i32),
								crdt_operation::record_id::equals(
									rmp_serde::to_vec(&op.record_id).unwrap(),
								),
								crdt_operation::kind::equals(OperationKind::Create.to_string()),
							])
							.order_by(crdt_operation::timestamp::order(SortOrder::Desc)),
						db.crdt_operation()
							.find_first(vec![
								crdt_operation::timestamp::gt(op.timestamp.as_u64() as i64),
								crdt_operation::model::equals(op.model as i32),
								crdt_operation::record_id::equals(
									rmp_serde::to_vec(&op.record_id).unwrap(),
								),
								crdt_operation::kind::equals(
									OperationKind::Update(field).to_string(),
								),
							])
							.order_by(crdt_operation::timestamp::order(SortOrder::Desc)),
					))
					.await?;

				// we don't care about the contents of the create operation, just that it exists
				// - all update operations come after creates, no check is necessary
				if create.is_none() || update.is_some() {
					return Ok(());
				}
			}
			// deletes are the be all and end all, no need to check anything
			sd_sync::CRDTOperationData::Delete => {}
		};

		// we don't want these writes to not apply together!
		self.db
			._transaction()
			.with_timeout(30 * 1000)
			.run(|db| async move {
				// apply the operation to the actual record
				ModelSyncData::from_op(op.clone())
					.unwrap()
					.exec(&db)
					.await
					.unwrap();

				// write the operation to the operations table
				write_crdt_op_to_db(&op, &db).await
			})
			.await?;

		// update the stored timestamp for this instance - will be derived from the crdt operations table on restart
		let new_ts = NTP64::max(timestamp.unwrap_or_default(), op_timestamp);
		self.timestamps.write().await.insert(op_instance, new_ts);

		self.io.req_tx.send(Request::Ingested).await.ok();

		Ok(())
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
	pub req_rx: Arc<Mutex<mpsc::Receiver<Request>>>,
}

#[derive(Debug)]
pub struct MessagesEvent {
	pub instance_id: Uuid,
	pub messages: Vec<CRDTOperation>,
	pub has_more: bool,
}

impl ActorTypes for Actor {
	type Event = Event;
	type Request = Request;
	type Handler = Handler;
}

#[cfg(test)]
mod test {
	use std::{sync::atomic::AtomicBool, time::Duration};

	use uhlc::HLCBuilder;

	use super::*;

	async fn new_actor() -> (Handler, Arc<SharedState>) {
		let instance = uuid::Uuid::new_v4();
		let shared = Arc::new(SharedState {
			db: sd_prisma::test_db().await,
			instance,
			clock: HLCBuilder::new().with_id(instance.into()).build(),
			timestamps: Default::default(),
			emit_messages_flag: Arc::new(AtomicBool::new(true)),
			active: Default::default(),
			active_notify: Default::default(),
		});

		(Actor::spawn(shared.clone()), shared)
	}

	/// If messages tx is dropped, actor should reset and assume no further messages
	/// will be sent
	#[tokio::test]
	async fn messages_request_drop() -> Result<(), ()> {
		let (ingest, _) = new_actor().await;

		for _ in 0..10 {
			let mut rx = ingest.req_rx.lock().await;

			ingest.event_tx.send(Event::Notification).await.unwrap();

			let Some(Request::Messages { .. }) = rx.recv().await else {
				panic!("bruh")
			};

			// without this the test hangs, idk
			tokio::time::sleep(Duration::from_millis(0)).await;
		}

		Ok(())
	}
}
