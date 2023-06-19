#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

use crate::prisma::*;

use std::{collections::HashMap, sync::Arc};

use sd_sync::*;

use serde_json::{json, to_vec, Value};
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
	node: Uuid,
	_clocks: HashMap<Uuid, NTP64>,
	clock: HLC,
	pub tx: Sender<SyncMessage>,
}

impl SyncManager {
	pub fn new(db: &Arc<PrismaClient>, node: Uuid) -> (Self, Receiver<SyncMessage>) {
		let (tx, rx) = broadcast::channel(64);

		(
			Self {
				db: db.clone(),
				node,
				clock: HLCBuilder::new().with_id(node.into()).build(),
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
							node::pub_id::equals(op.node.as_bytes().to_vec()),
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
							node::pub_id::equals(op.node.as_bytes().to_vec()),
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
			.include(shared_operation::include!({ node: select {
                pub_id
            } }))
			.exec()
			.await?
			.into_iter()
			.flat_map(|op| {
				Some(CRDTOperation {
					id: Uuid::from_slice(&op.id).ok()?,
					node: Uuid::from_slice(&op.node.pub_id).ok()?,
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
			.node()
			.find_unique(node::pub_id::equals(op.node.as_bytes().to_vec()))
			.exec()
			.await?
			.is_none()
		{
			panic!("Node is not paired!")
		}

		let msg = SyncMessage::Ingested(op.clone());

		match ModelSyncData::from_op(op.typ.clone()).unwrap() {
			ModelSyncData::FilePath(id, shared_op) => match shared_op {
				SharedOperationData::Create(data) => {
					let data: Vec<_> = data
						.into_iter()
						.flat_map(|(k, v)| file_path::SetParam::deserialize(&k, v))
						.collect();

					db.file_path()
						.upsert(
							file_path::pub_id::equals(id.pub_id.clone()),
							file_path::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				SharedOperationData::Update { field, value } => {
					let data = vec![file_path::SetParam::deserialize(&field, value).unwrap()];

					db.file_path()
						.upsert(
							file_path::pub_id::equals(id.pub_id.clone()),
							file_path::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Location(id, shared_op) => match shared_op {
				SharedOperationData::Create(data) => {
					let data: Vec<_> = data
						.into_iter()
						.flat_map(|(k, v)| location::SetParam::deserialize(&k, v))
						.collect();

					db.location()
						.upsert(
							location::pub_id::equals(id.pub_id.clone()),
							location::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				SharedOperationData::Update { field, value } => {
					let data = vec![location::SetParam::deserialize(&field, value).unwrap()];

					db.location()
						.upsert(
							location::pub_id::equals(id.pub_id.clone()),
							location::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Object(id, shared_op) => match shared_op {
				SharedOperationData::Create(data) => {
					let data: Vec<_> = data
						.into_iter()
						.flat_map(|(k, v)| object::SetParam::deserialize(&k, v))
						.collect();

					db.object()
						.upsert(
							object::pub_id::equals(id.pub_id.clone()),
							object::create(id.pub_id, vec![]),
							data,
						)
						.exec()
						.await?;
				}
				SharedOperationData::Update { field, value } => {
					let data = vec![object::SetParam::deserialize(&field, value).unwrap()];

					db.object()
						.upsert(
							object::pub_id::equals(id.pub_id.clone()),
							object::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Tag(id, shared_op) => match shared_op {
				SharedOperationData::Create(data) => {
					let data: Vec<_> = data
						.into_iter()
						.flat_map(|(field, value)| tag::SetParam::deserialize(&field, value))
						.collect();

					db.tag()
						.upsert(
							tag::pub_id::equals(id.pub_id.clone()),
							tag::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				SharedOperationData::Update { field, value } => {
					let data = vec![tag::SetParam::deserialize(&field, value).unwrap()];

					db.tag()
						.upsert(
							tag::pub_id::equals(id.pub_id.clone()),
							tag::create(id.pub_id, data.clone()),
							data,
						)
						.exec()
						.await?;
				}
				SharedOperationData::Delete => {
					db.tag()
						.delete(tag::pub_id::equals(id.pub_id))
						.exec()
						.await?;
				}
			},
		}

		if let CRDTOperationType::Shared(shared_op) = op.typ {
			let kind = match &shared_op.data {
				SharedOperationData::Create(_) => "c",
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
					node::pub_id::equals(op.node.as_bytes().to_vec()),
					vec![],
				)
				.exec()
				.await?;
		}

		self.tx.send(msg).ok();

		Ok(())
	}

	fn new_op(&self, typ: CRDTOperationType) -> CRDTOperation {
		let timestamp = self.clock.new_timestamp();

		CRDTOperation {
			node: self.node,
			timestamp: *timestamp.get_time(),
			id: Uuid::new_v4(),
			typ,
		}
	}

	pub fn unique_shared_create<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, Value)> + 'static,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Create(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		}))
	}
	pub fn shared_update<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
		field: &str,
		value: Value,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Update {
				field: field.to_string(),
				value,
			},
		}))
	}
}
