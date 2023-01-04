use sd_sync::*;
use serde::Deserialize;
use serde_json::{from_slice, from_value, to_vec, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uhlc::{HLCBuilder, HLC, NTP64};
use uuid::Uuid;

use crate::prisma::{
	file_path, location, node, object, owned_operation, shared_operation, PrismaClient,
};

pub struct SyncManager {
	db: Arc<PrismaClient>,
	node: Uuid,
	_clocks: HashMap<Uuid, NTP64>,
	clock: HLC,
	tx: Sender<CRDTOperation>,
}

impl SyncManager {
	pub fn new(db: Arc<PrismaClient>, node: Uuid) -> (Self, Receiver<CRDTOperation>) {
		let (tx, rx) = mpsc::channel(64);

		(
			Self {
				db,
				node,
				clock: HLCBuilder::new().with_id(node.into()).build(),
				_clocks: Default::default(),
				tx,
			},
			rx,
		)
	}

	pub async fn write_ops<'item, Q: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		ops: Vec<CRDTOperation>,
		queries: Q,
	) -> prisma_client_rust::Result<<Q as prisma_client_rust::BatchItemParent>::ReturnValue> {
		let owned = ops
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

		let shared = ops
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

		for op in ops {
			self.tx.send(op).await.ok();
		}

		Ok(res)
	}

	pub async fn write_op<'query, Q: prisma_client_rust::Query<'query>>(
		&self,
		tx: &PrismaClient,
		op: CRDTOperation,
		query: Q,
	) -> prisma_client_rust::Result<Q::ReturnValue> {
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

		self.tx.send(op).await.ok();

		Ok(ret)
	}

	pub async fn get_ops(&self) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		owned_operation::include!(owned_op_with_node { node });

		impl TryInto<CRDTOperation> for owned_op_with_node::Data {
			type Error = ();

			fn try_into(self) -> Result<CRDTOperation, Self::Error> {
				let id = Uuid::from_slice(&self.id).map_err(|_| ())?;
				let node = Uuid::from_slice(&self.node.pub_id).map_err(|_| ())?;

				Ok(CRDTOperation {
					id,
					node,
					timestamp: NTP64(self.timestamp as u64),
					typ: CRDTOperationType::Owned(OwnedOperation {
						model: self.model,
						items: serde_json::from_slice(&self.data).unwrap(),
					}),
				})
			}
		}

		shared_operation::include!(shared_op_with_node { node });

		impl TryInto<CRDTOperation> for shared_op_with_node::Data {
			type Error = ();

			fn try_into(self) -> Result<CRDTOperation, Self::Error> {
				let id = Uuid::from_slice(&self.id).map_err(|_| ())?;
				let node = Uuid::from_slice(&self.node.pub_id).map_err(|_| ())?;

				Ok(CRDTOperation {
					id,
					node,
					timestamp: NTP64(self.timestamp as u64),
					typ: CRDTOperationType::Shared(SharedOperation {
						record_id: serde_json::from_slice(&self.record_id).unwrap(),
						model: self.model,
						data: from_slice(&self.data).unwrap(),
					}),
				})
			}
		}

		let owned = self
			.db
			.owned_operation()
			.find_many(vec![])
			.include(owned_op_with_node::include())
			.exec()
			.await?
			.into_iter()
			.map(TryInto::try_into);
		let shared = self
			.db
			.shared_operation()
			.find_many(vec![])
			.include(shared_op_with_node::include())
			.exec()
			.await?
			.into_iter()
			.map(TryInto::try_into);

		let mut result: Vec<CRDTOperation> = owned.chain(shared).flatten().collect();

		result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

		Ok(result)
	}

	pub async fn ingest_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		match op.typ {
			CRDTOperationType::Owned(owned_op) => match owned_op.model.as_str() {
				"FilePath" => {
					#[derive(Deserialize)]
					struct FilePathId {
						location_id: Vec<u8>,
						id: i32,
					}

					for item in owned_op.items {
						let id: FilePathId = serde_json::from_value(item.id).unwrap();

						let location = self
							.db
							.location()
							.find_unique(location::pub_id::equals(id.location_id))
							.select(location::select!({ id }))
							.exec()
							.await?
							.unwrap();

						match item.data {
							OwnedOperationData::Create(mut data) => {
								self.db
									.file_path()
									.create(
										id.id,
										location::id::equals(location.id),
										serde_json::from_value(
											data.remove("materialized_path").unwrap(),
										)
										.unwrap(),
										serde_json::from_value(data.remove("name").unwrap())
											.unwrap(),
										data.into_iter()
											.map(|(k, v)| file_path::SetParam::deserialize(&k, v))
											.flatten()
											.collect(),
									)
									.exec()
									.await?;
							}
							OwnedOperationData::Update(data) => {
								self.db
									.file_path()
									.update(
										file_path::location_id_id(location.id, id.id),
										data.into_iter()
											.map(|(k, v)| file_path::SetParam::deserialize(&k, v))
											.flatten()
											.collect(),
									)
									.exec()
									.await?;
							}
							_ => todo!(),
						}
					}
				}
				"Location" => {
					#[derive(Deserialize)]
					struct LocationId {
						id: Vec<u8>,
					}

					for item in owned_op.items {
						let id: LocationId = serde_json::from_value(item.id).unwrap();

						match item.data {
							OwnedOperationData::Create(mut data) => {
								self.db
									.location()
									.create(
										id.id,
										{
											let val: std::collections::HashMap<String, Value> =
												from_value(data.remove("node").unwrap()).unwrap();
											let val = val.into_iter().next().unwrap();

											node::UniqueWhereParam::deserialize(&val.0, val.1)
												.unwrap()
										},
										data.into_iter()
											.map(|(k, v)| location::SetParam::deserialize(&k, v))
											.flatten()
											.collect(),
									)
									.exec()
									.await?;
							}
							_ => todo!(),
						}
					}
				}
				_ => {}
			},
			CRDTOperationType::Shared(shared_op) => match shared_op.model.as_str() {
				"Object" => {
					let cas_id: String = from_value(shared_op.record_id).unwrap();

					match shared_op.data {
						SharedOperationData::Create(_) => {
							self.db
								.object()
								.upsert(
									object::cas_id::equals(cas_id.clone()),
									(cas_id, vec![]),
									vec![],
								)
								.exec()
								.await
								.ok();
						}
						SharedOperationData::Update { field, value } => {
							self.db
								.object()
								.update(
									object::cas_id::equals(cas_id),
									vec![object::SetParam::deserialize(&field, value).unwrap()],
								)
								.exec()
								.await?;
						}
						_ => todo!(),
					}
				}
				_ => todo!(),
			},
			_ => {}
		}

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

	pub fn owned_create<const SIZE: usize>(
		&self,
		model: &str,
		id: Value,
		values: [(&'static str, Value); SIZE],
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: model.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id,
					data: OwnedOperationData::Create(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}

	pub fn owned_create_many<const SIZE: usize>(
		&self,
		model: &str,
		data: impl IntoIterator<Item = (Value, [(&'static str, Value); SIZE])>,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: model.to_string(),
			items: data
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id,
					data: OwnedOperationData::Create(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}

	pub fn owned_update<const SIZE: usize>(
		&self,
		model: &str,
		id: Value,
		values: [(&'static str, Value); SIZE],
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: model.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id,
					data: OwnedOperationData::Update(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}

	pub fn shared_create(&self, model: &str, record_id: Value) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: model.to_string(),
			record_id,
			data: SharedOperationData::Create(SharedOperationCreateData::Atomic),
		}))
	}

	pub fn shared_update(
		&self,
		model: &str,
		record_id: Value,
		field: &str,
		value: Value,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: model.to_string(),
			record_id,
			data: SharedOperationData::Update {
				field: field.to_string(),
				value,
			},
		}))
	}
}
