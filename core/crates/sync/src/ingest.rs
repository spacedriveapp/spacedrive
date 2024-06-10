use std::{
	collections::BTreeMap,
	num::NonZeroU128,
	ops::Deref,
	sync::{atomic::Ordering, Arc},
};

use sd_prisma::{
	prisma::{crdt_operation, SortOrder},
	prisma_sync::ModelSyncData,
};
use sd_sync::{
	CRDTOperation, CRDTOperationData, CompressedCRDTOperation, CompressedCRDTOperations,
	OperationKind,
};
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
	// Ingested,
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

				wait!(self.io.event_rx.lock().await, Event::Notification);

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

				let mut event_rx = self.io.event_rx.lock().await;

				loop {
					tokio::select! {
						biased;
						res = event_rx.recv() => {
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
				debug!(
					messages_count = event.messages.len(),
					first_message = event.messages.first().unwrap().3.timestamp.as_u64(),
					last_message = event.messages.last().unwrap().3.timestamp.as_u64(),
					"Ingesting operations;",
				);

				for (instance, data) in event.messages.0 {
					for (model, data) in data {
						for (record, ops) in data {
							self.receive_crdt_operations(instance, model, record, ops)
								.await
								.expect("sync ingest failed");
						}
					}
				}

				if let Some(tx) = event.wait_tx {
					tx.send(()).ok();
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

	pub async fn declare(shared: Arc<SharedState>) -> Handler {
		let (actor_io, handler_io) = create_actor_io::<Self>();

		shared
			.actors
			.declare(
				"Sync Ingest",
				{
					let shared = shared.clone();
					move || async move {
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
					}
				},
				true,
			)
			.await;

		Handler {
			event_tx: handler_io.event_tx,
			req_rx: Arc::new(Mutex::new(handler_io.req_rx)),
		}
	}

	// where the magic happens
	async fn receive_crdt_operations(
		&mut self,
		instance: Uuid,
		model: u16,
		record_id: rmpv::Value,
		mut ops: Vec<CompressedCRDTOperation>,
	) -> prisma_client_rust::Result<()> {
		let db = &self.db;

		ops.sort_by_key(|op| op.timestamp);

		let new_timestamp = ops.last().expect("Empty ops array").timestamp;

		// first, we update the HLC's timestamp with the incoming one.
		// this involves a drift check + sets the last time of the clock
		self.clock
			.update_with_timestamp(&Timestamp::new(
				new_timestamp,
				uhlc::ID::from(NonZeroU128::new(instance.to_u128_le()).expect("Non zero id")),
			))
			.expect("timestamp has too much drift!");

		// read the timestamp for the operation's instance, or insert one if it doesn't exist
		let timestamp = self.timestamps.read().await.get(&instance).cloned();

		// Delete - ignores all other messages
		if let Some(delete_op) = ops
			.iter()
			.rev()
			.find(|op| matches!(op.data, sd_sync::CRDTOperationData::Delete))
		{
			// deletes are the be all and end all, no need to check anything

			let op = CRDTOperation {
				instance,
				model,
				record_id,
				timestamp: delete_op.timestamp,
				data: CRDTOperationData::Delete,
			};

			self.db
				._transaction()
				.with_timeout(30 * 1000)
				.run(|db| async move {
					ModelSyncData::from_op(op.clone())
						.unwrap()
						.exec(&db)
						.await?;
					write_crdt_op_to_db(&op, &db).await?;

					Ok(())
				})
				.await?;
		}
		// Create + > 0 Update - overwrites the create's data with the updates
		else if let Some(timestamp) = ops.iter().rev().find_map(|op| {
			if let sd_sync::CRDTOperationData::Create(_) = &op.data {
				return Some(op.timestamp);
			}

			None
		}) {
			// conflict resolution
			let delete = db
				.crdt_operation()
				.find_first(vec![
					crdt_operation::model::equals(model as i32),
					crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id).unwrap()),
					crdt_operation::kind::equals(OperationKind::Delete.to_string()),
				])
				.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
				.exec()
				.await?;

			if delete.is_some() {
				return Ok(());
			}

			let mut data = BTreeMap::new();

			let mut applied_ops = vec![];

			// search for all Updates until a Create is found
			for op in ops.iter().rev() {
				match &op.data {
					CRDTOperationData::Delete => unreachable!("Delete can't exist here!"),
					CRDTOperationData::Create(create_data) => {
						for (k, v) in create_data {
							data.entry(k).or_insert(v);
						}

						applied_ops.push(op);

						break;
					}
					CRDTOperationData::Update { field, value } => {
						applied_ops.push(op);
						data.insert(field, value);
					}
				}
			}

			self.db
				._transaction()
				.with_timeout(30 * 1000)
				.run(|db| async move {
					// fake a create with a bunch of data rather than individual insert
					ModelSyncData::from_op(CRDTOperation {
						instance,
						model,
						record_id: record_id.clone(),
						timestamp,
						data: CRDTOperationData::Create(
							data.into_iter()
								.map(|(k, v)| (k.clone(), v.clone()))
								.collect(),
						),
					})
					.unwrap()
					.exec(&db)
					.await?;

					for op in applied_ops {
						write_crdt_op_to_db(
							&CRDTOperation {
								instance,
								model,
								record_id: record_id.clone(),
								timestamp: op.timestamp,
								data: op.data.clone(),
							},
							&db,
						)
						.await?;
					}

					Ok(())
				})
				.await?;
		}
		// > 0 Update - batches updates with a fake Create op
		else {
			let mut data = BTreeMap::new();

			for op in ops.into_iter().rev() {
				let CRDTOperationData::Update { field, value } = op.data else {
					unreachable!("Create + Delete should be filtered out!");
				};

				data.insert(field, (value, op.timestamp));
			}

			// conflict resolution
			let (create, updates) = db
				._batch((
					db.crdt_operation()
						.find_first(vec![
							crdt_operation::model::equals(model as i32),
							crdt_operation::record_id::equals(
								rmp_serde::to_vec(&record_id).unwrap(),
							),
							crdt_operation::kind::equals(OperationKind::Create.to_string()),
						])
						.order_by(crdt_operation::timestamp::order(SortOrder::Desc)),
					data.iter()
						.map(|(k, (_, timestamp))| {
							db.crdt_operation()
								.find_first(vec![
									crdt_operation::timestamp::gt(timestamp.as_u64() as i64),
									crdt_operation::model::equals(model as i32),
									crdt_operation::record_id::equals(
										rmp_serde::to_vec(&record_id).unwrap(),
									),
									crdt_operation::kind::equals(
										OperationKind::Update(k).to_string(),
									),
								])
								.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
						})
						.collect::<Vec<_>>(),
				))
				.await?;

			if create.is_none() {
				return Ok(());
			}

			// does the same thing as processing ops one-by-one and returning early if a newer op was found
			for (update, key) in updates
				.into_iter()
				.zip(data.keys().cloned().collect::<Vec<_>>())
			{
				if update.is_some() {
					data.remove(&key);
				}
			}

			self.db
				._transaction()
				.with_timeout(30 * 1000)
				.run(|db| async move {
					// fake operation to batch them all at once
					ModelSyncData::from_op(CRDTOperation {
						instance,
						model,
						record_id: record_id.clone(),
						timestamp: NTP64(0),
						data: CRDTOperationData::Create(
							data.iter()
								.map(|(k, (data, _))| (k.to_string(), data.clone()))
								.collect(),
						),
					})
					.unwrap()
					.exec(&db)
					.await?;

					// need to only apply ops that haven't been filtered out
					for (field, (value, timestamp)) in data {
						write_crdt_op_to_db(
							&CRDTOperation {
								instance,
								model,
								record_id: record_id.clone(),
								timestamp,
								data: CRDTOperationData::Update { field, value },
							},
							&db,
						)
						.await?;
					}

					Ok(())
				})
				.await?;
		}

		// update the stored timestamp for this instance - will be derived from the crdt operations table on restart
		let new_ts = NTP64::max(timestamp.unwrap_or_default(), new_timestamp);

		self.timestamps.write().await.insert(instance, new_ts);

		// self.io.req_tx.send(Request::Ingested).await.ok();

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
	pub messages: CompressedCRDTOperations,
	pub has_more: bool,
	pub wait_tx: Option<oneshot::Sender<()>>,
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
			clock: HLCBuilder::new()
				.with_id(uhlc::ID::from(
					NonZeroU128::new(instance.to_u128_le()).expect("Non zero id"),
				))
				.build(),
			timestamps: Default::default(),
			emit_messages_flag: Arc::new(AtomicBool::new(true)),
			active: Default::default(),
			active_notify: Default::default(),
			actors: Default::default(),
		});

		(Actor::declare(shared.clone()).await, shared)
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
