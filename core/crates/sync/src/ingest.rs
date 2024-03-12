use std::{ops::Deref, sync::Arc};

use sd_prisma::{
	prisma::{crdt_operation, instance, PrismaClient, SortOrder},
	prisma_sync::ModelSyncData,
};
use sd_sync::CRDTOperation;
use serde_json::to_vec;
use tokio::sync::{mpsc, Mutex};
use uhlc::{Timestamp, NTP64};
use uuid::Uuid;

use crate::{
	actor::{create_actor_io, ActorIO, ActorTypes},
	wait, SharedState,
};

#[derive(Debug)]
#[must_use]
/// Stuff that can be handled outside the actor
pub enum Request {
	Messages { timestamps: Vec<(Uuid, NTP64)> },
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
				wait!(self.io.event_rx, Event::Notification);

				State::RetrievingMessages
			}
			State::RetrievingMessages => {
				self.io
					.send(Request::Messages {
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
				for op in event.messages {
					let fut = self.receive_crdt_operation(op);
					fut.await;
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
	async fn receive_crdt_operation(&mut self, op: CRDTOperation) {
		// first, we update the HLC's timestamp with the incoming one.
		// this involves a drift check + sets the last time of the clock
		self.clock
			.update_with_timestamp(&Timestamp::new(op.timestamp, op.instance.into()))
			.expect("timestamp has too much drift!");

		// read the timestamp for the operation's instance, or insert one if it doesn't exist
		let timestamp = self.timestamps.write().await.get(&op.instance).cloned();

		// copy some fields bc rust ownership
		let op_instance = op.instance;
		let op_timestamp = op.timestamp;

		if !self.is_operation_old(&op).await {
			// actually go and apply the operation in the db
			self.apply_op(op).await.ok();

			// update the stored timestamp for this instance - will be derived from the crdt operations table on restart
			self.timestamps.write().await.insert(
				op_instance,
				NTP64::max(timestamp.unwrap_or_default(), op_timestamp),
			);
		}
	}

	async fn apply_op(&mut self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		self.db
			._transaction()
			.run(|db| async move {
				// apply the operation to the actual record
				ModelSyncData::from_op(op.clone())
					.unwrap()
					.exec(&db)
					.await?;

				// write the operation to the operations table
				write_crdt_op_to_db(&op, &db).await?;

				Ok(())
			})
			.await?;

		self.io.req_tx.send(Request::Ingested).await.ok();

		Ok(())
	}

	// determines if an operation is old and shouldn't be applied
	async fn is_operation_old(&mut self, op: &CRDTOperation) -> bool {
		let db = &self.db;

		let old_timestamp = {
			let newer_op = db
				.crdt_operation()
				.find_first(vec![
					crdt_operation::timestamp::gte(op.timestamp.as_u64() as i64),
					crdt_operation::model::equals(op.model.to_string()),
					crdt_operation::record_id::equals(serde_json::to_vec(&op.record_id).unwrap()),
					crdt_operation::kind::equals(op.kind().to_string()),
				])
				.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
				.exec()
				.await
				.unwrap();

			newer_op.map(|newer_op| newer_op.timestamp)
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

async fn write_crdt_op_to_db(
	op: &CRDTOperation,
	db: &PrismaClient,
) -> Result<(), prisma_client_rust::QueryError> {
	crdt_op_db(op).to_query(db).exec().await?;

	Ok(())
}

fn crdt_op_db(op: &CRDTOperation) -> crdt_operation::Create {
	crdt_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.kind().to_string(),
		data: to_vec(&op.data).unwrap(),
		model: op.model.to_string(),
		record_id: to_vec(&op.record_id).unwrap(),
		_params: vec![],
	}
}
