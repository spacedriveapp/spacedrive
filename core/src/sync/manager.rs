use crate::prisma::*;

use std::{collections::HashMap, sync::Arc};

use prisma_client_rust::Direction;
use sd_sync::*;

use serde_json::{from_value, json, to_vec, Value};
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
			let owned = _ops
				.iter()
				.filter_map(|op| match &op.typ {
					CRDTOperationType::Owned(owned_op) => Some(tx.owned_operation().create(
						op.id.as_bytes().to_vec(),
						op.timestamp.0 as i64,
						to_vec(&owned_op.items).unwrap(),
						owned_op.model.clone(),
						node::pub_id::equals(op.node.as_bytes().to_vec()),
						vec![],
					)),
					_ => None,
				})
				.collect::<Vec<_>>();

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

			let (res, _) = tx._batch((queries, (owned, shared))).await?;

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
		let ret = match &op.typ {
			CRDTOperationType::Owned(owned_op) => {
				tx._batch((
					tx.owned_operation().create(
						op.id.as_bytes().to_vec(),
						op.timestamp.0 as i64,
						to_vec(&owned_op.items).unwrap(),
						owned_op.model.clone(),
						node::pub_id::equals(op.node.as_bytes().to_vec()),
						vec![],
					),
					query,
				))
				.await?
				.1
			}
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

		Ok(ret)
	}

	pub async fn get_ops(&self) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		Ok(self
			.db
			.shared_operation()
			.find_many(vec![])
			.order_by(shared_operation::timestamp::order(Direction::Asc))
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

		db.node()
			.upsert(
				node::pub_id::equals(op.node.as_bytes().to_vec()),
				node::create(op.node.as_bytes().to_vec(), "TEMP".to_string(), vec![]),
				vec![],
			)
			.exec()
			.await?;

		let msg = SyncMessage::Ingested(op.clone());

		match ModelSyncData::from_op(op.typ.clone()).unwrap() {
			ModelSyncData::FilePath(id, shared_op) => match shared_op {
				SharedOperationData::Create(SharedOperationCreateData::Unique(mut data)) => {
					db.file_path()
						.create(
							id.pub_id,
							{
								let val: std::collections::HashMap<String, Value> =
									from_value(data.remove(file_path::location::NAME).unwrap())
										.unwrap();
								let val = val.into_iter().next().unwrap();

								location::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
							},
							serde_json::from_value(
								data.remove(file_path::materialized_path::NAME).unwrap(),
							)
							.unwrap(),
							serde_json::from_value(data.remove(file_path::name::NAME).unwrap())
								.unwrap(),
							serde_json::from_value(
								data.remove(file_path::extension::NAME)
									.unwrap_or_else(|| serde_json::Value::String("".to_string())),
							)
							.unwrap(),
							serde_json::from_value(data.remove(file_path::inode::NAME).unwrap())
								.unwrap(),
							serde_json::from_value(data.remove(file_path::device::NAME).unwrap())
								.unwrap(),
							data.into_iter()
								.flat_map(|(k, v)| file_path::SetParam::deserialize(&k, v))
								.collect(),
						)
						.exec()
						.await?;
				}
				SharedOperationData::Update { field, value } => {
					self.db
						.file_path()
						.update(
							file_path::pub_id::equals(id.pub_id),
							vec![file_path::SetParam::deserialize(&field, value).unwrap()],
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Location(id, shared_op) => match shared_op {
				SharedOperationData::Create(SharedOperationCreateData::Unique(mut data)) => {
					db.location()
						.create(
							id.pub_id,
							serde_json::from_value(data.remove(location::name::NAME).unwrap())
								.unwrap(),
							serde_json::from_value(data.remove(location::path::NAME).unwrap())
								.unwrap(),
							{
								let val: std::collections::HashMap<String, Value> =
									from_value(data.remove(location::node::NAME).unwrap()).unwrap();
								let val = val.into_iter().next().unwrap();

								node::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
							},
							data.into_iter()
								.flat_map(|(k, v)| location::SetParam::deserialize(&k, v))
								.collect(),
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Object(id, shared_op) => match shared_op {
				SharedOperationData::Create(_) => {
					db.object()
						.upsert(
							object::pub_id::equals(id.pub_id.clone()),
							object::create(id.pub_id, vec![]),
							vec![],
						)
						.exec()
						.await
						.ok();
				}
				SharedOperationData::Update { field, value } => {
					db.object()
						.update(
							object::pub_id::equals(id.pub_id),
							vec![object::SetParam::deserialize(&field, value).unwrap()],
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Tag(id, shared_op) => match shared_op {
				SharedOperationData::Create(create_data) => match create_data {
					SharedOperationCreateData::Unique(create_data) => {
						db.tag()
							.create(
								id.pub_id,
								create_data
									.into_iter()
									.flat_map(|(field, value)| {
										tag::SetParam::deserialize(&field, value)
									})
									.collect(),
							)
							.exec()
							.await?;
					}
					_ => unreachable!(),
				},
				SharedOperationData::Update { field, value } => {
					db.tag()
						.update(
							tag::pub_id::equals(id.pub_id),
							vec![tag::SetParam::deserialize(&field, value).unwrap()],
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
			_ => todo!(),
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

	pub fn owned_create<
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: [(&'static str, Value); SIZE],
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id: json!(id),
					data: OwnedOperationData::Create(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}
	pub fn owned_create_many<
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		data: impl IntoIterator<Item = (TSyncId, [(&'static str, Value); SIZE])>,
		skip_duplicates: bool,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: vec![OwnedOperationItem {
				id: Value::Null,
				data: OwnedOperationData::CreateMany {
					values: data
						.into_iter()
						.map(|(id, data)| {
							(
								json!(id),
								data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
							)
						})
						.collect(),
					skip_duplicates,
				},
			}],
		}))
	}
	pub fn owned_update<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, Value)>,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id: json!(id),
					data: OwnedOperationData::Update(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}

	pub fn shared_create<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Create(SharedOperationCreateData::Atomic),
		}))
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
			data: SharedOperationData::Create(SharedOperationCreateData::Unique(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			)),
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
