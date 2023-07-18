#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

use crate::{prisma::*, util::db::uuid_to_bytes};
use sd_sync::*;

use std::{collections::HashMap, sync::Arc};

use serde_json::to_vec;
use tokio::sync::broadcast::{self, Receiver, Sender};
use uhlc::{HLCBuilder, Timestamp, HLC, NTP64};
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
			macro_rules! variant {
				($var:ident, $variant:ident, $fn:ident) => {
					let $var = _ops
						.iter()
						.filter_map(|op| match &op.typ {
							CRDTOperationType::$variant(inner) => {
								Some($fn(&op, &inner).to_query(tx))
							}
							_ => None,
						})
						.collect::<Vec<_>>();
				};
			}

			variant!(shared, Shared, shared_op_db);
			variant!(relation, Relation, relation_op_db);

			let (res, _) = tx._batch((queries, (shared, relation))).await?;

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
			macro_rules! exec {
				($fn:ident, $inner:ident) => {
					tx._batch(($fn(&op, $inner).to_query(tx), query)).await?.1
				};
			}

			let ret = match &op.typ {
				CRDTOperationType::Shared(inner) => exec!(shared_op_db, inner),
				CRDTOperationType::Relation(inner) => exec!(relation_op_db, inner),
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
			.order_by(shared_operation::timestamp::order(SortOrder::Desc))
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

	pub async fn apply_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		ModelSyncData::from_op(op.typ.clone())
			.unwrap()
			.exec(&self.db)
			.await?;

		match &op.typ {
			CRDTOperationType::Shared(shared_op) => {
				shared_op_db(&op, &shared_op)
					.to_query(&self.db)
					.exec()
					.await?;
			}
			CRDTOperationType::Relation(relation_op) => {
				relation_op_db(&op, &relation_op)
					.to_query(&self.db)
					.exec()
					.await?;
			}
		}

		self.tx.send(SyncMessage::Ingested(op.clone())).ok();

		Ok(())
	}

	async fn compare_message(&self, op: &CRDTOperation) -> bool {
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

		let is_old = old_timestamp
			.map(|old| old != op.timestamp.as_u64() as i64)
			.unwrap_or_default();

		is_old
	}

	pub async fn receive_crdt_operation(&mut self, op: CRDTOperation) {
		self.clock
			.update_with_timestamp(&Timestamp::new(op.timestamp, op.instance.into()))
			.ok();

		let timestamp = self
			._clocks
			.entry(op.instance)
			.or_insert_with(|| op.timestamp);

		if *timestamp < op.timestamp {
			*timestamp = op.timestamp;
		}

		let op_timestamp = op.timestamp;
		let op_instance = op.instance;

		let is_old = self.compare_message(&op).await;

		if !is_old {
			self.apply_op(op).await.ok();
		}

		self.db
			.instance()
			.update(
				instance::pub_id::equals(uuid_to_bytes(op_instance)),
				vec![instance::timestamp::set(op_timestamp.as_u64() as i64)],
			)
			.exec()
			.await
			.ok();
	}
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

impl OperationFactory for SyncManager {
	fn get_clock(&self) -> &HLC {
		&self.clock
	}

	fn get_instance(&self) -> Uuid {
		self.instance
	}
}
