#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

use crate::prisma::*;
use sd_sync::*;

use std::{collections::HashMap, sync::Arc};

use serde_json::to_vec;
use tokio::sync::broadcast::{self, Receiver, Sender};
use uhlc::{HLCBuilder, HLC, NTP64};
use uuid::Uuid;

use super::ModelSyncData;

#[derive(Clone)]
pub enum SyncMessage {
	Ingested(CRDTOperation),
	Created(CRDTOperation),
}

pub struct SyncManager {
	db: Arc<PrismaClient>,
	instance: Uuid,
	_clocks: HashMap<Uuid, NTP64>,
	clock: HLC,
	pub tx: Sender<SyncMessage>,
}

impl SyncManager {
	pub fn new(db: &Arc<PrismaClient>, instance: Uuid) -> (Self, Receiver<SyncMessage>) {
		let (tx, rx) = broadcast::channel(64);

		(
			Self {
				db: db.clone(),
				instance,
				clock: HLCBuilder::new().with_id(instance.into()).build(),
				_clocks: Default::default(),
				tx,
			},
			rx,
		)
	}

	pub async fn write_ops<'item, I: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		(_ops, queries): (Vec<CRDTOperation>, I),
	) -> prisma_client_rust::Result<<I as prisma_client_rust::BatchItemParent>::ReturnValue> {
		#[cfg(feature = "sync-messages")]
		let res = {
			let shared = _ops
				.iter()
				.filter_map(|op| match &op.typ {
					CRDTOperationType::Shared(shared_op) => {
						let kind = match &shared_op.data {
							SharedOperationData::Create(_) => "c",
							SharedOperationData::Update { .. } => "u",
							SharedOperationData::Delete => "d",
						};

						Some(tx.shared_operation().create(
							op.id.as_bytes().to_vec(),
							op.timestamp.0 as i64,
							shared_op.model.to_string(),
							to_vec(&shared_op.record_id).unwrap(),
							kind.to_string(),
							to_vec(&shared_op.data).unwrap(),
							instance::pub_id::equals(op.instance.as_bytes().to_vec()),
							vec![],
						))
					}
					_ => None,
				})
				.collect::<Vec<_>>();

			let (res, _) = tx._batch((queries, shared)).await?;

			for op in _ops {
				self.tx.send(SyncMessage::Created(op)).ok();
			}

			res
		};
		#[cfg(not(feature = "sync-messages"))]
		let res = tx._batch([queries]).await?.remove(0);

		Ok(res)
	}

	#[allow(unused_variables)]
	pub async fn write_op<'item, Q: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		op: CRDTOperation,
		query: Q,
	) -> prisma_client_rust::Result<<Q as prisma_client_rust::BatchItemParent>::ReturnValue> {
		#[cfg(feature = "sync-messages")]
		let ret = {
			let ret = match &op.typ {
				CRDTOperationType::Shared(shared_op) => {
					let kind = match &shared_op.data {
						SharedOperationData::Create(_) => "c",
						SharedOperationData::Update { .. } => "u",
						SharedOperationData::Delete => "d",
					};

					tx._batch((
						tx.shared_operation().create(
							op.id.as_bytes().to_vec(),
							op.timestamp.0 as i64,
							shared_op.model.to_string(),
							to_vec(&shared_op.record_id).unwrap(),
							kind.to_string(),
							to_vec(&shared_op.data).unwrap(),
							instance::pub_id::equals(op.instance.as_bytes().to_vec()),
							vec![],
						),
						query,
					))
					.await?
					.1
				}
				_ => todo!(),
			};

			self.tx.send(SyncMessage::Created(op)).ok();

			ret
		};
		#[cfg(not(feature = "sync-messages"))]
		let ret = tx._batch(vec![query]).await?.remove(0);

		Ok(ret)
	}

	pub async fn get_ops(&self) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		Ok(self
			.db
			.shared_operation()
			.find_many(vec![])
			.order_by(shared_operation::timestamp::order(SortOrder::Asc))
			.include(shared_operation::include!({ instance: select {
                pub_id
            } }))
			.exec()
			.await?
			.into_iter()
			.flat_map(|op| {
				Some(CRDTOperation {
					id: Uuid::from_slice(&op.id).ok()?,
					instance: Uuid::from_slice(&op.instance.pub_id).ok()?,
					timestamp: NTP64(op.timestamp as u64),
					typ: CRDTOperationType::Shared(SharedOperation {
						record_id: serde_json::from_slice(&op.record_id).ok()?,
						model: op.model,
						data: serde_json::from_slice(&op.data).ok()?,
					}),
				})
			})
			.collect())
	}

	pub async fn ingest_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		let db = &self.db;

		if db
			.instance()
			.find_unique(instance::pub_id::equals(op.instance.as_bytes().to_vec()))
			.exec()
			.await?
			.is_none()
		{
			panic!("Node is not paired!")
		}

		let msg = SyncMessage::Ingested(op.clone());

		ModelSyncData::from_op(op.typ.clone())
			.unwrap()
			.exec(db)
			.await?;

		if let CRDTOperationType::Shared(shared_op) = op.typ {
			let kind = match &shared_op.data {
				SharedOperationData::Create => "c",
				SharedOperationData::Update { .. } => "u",
				SharedOperationData::Delete => "d",
			};

			db.shared_operation()
				.create(
					op.id.as_bytes().to_vec(),
					op.timestamp.0 as i64,
					shared_op.model.to_string(),
					to_vec(&shared_op.record_id).unwrap(),
					kind.to_string(),
					to_vec(&shared_op.data).unwrap(),
					instance::pub_id::equals(op.instance.as_bytes().to_vec()),
					vec![],
				)
				.exec()
				.await?;
		}

		self.tx.send(msg).ok();

		Ok(())
	}
}

impl OperationFactory for SyncManager {
	fn get_clock(&self) -> &HLC {
		&self.clock
	}

	fn get_instance(&self) -> Uuid {
		self.instance
	}
}
