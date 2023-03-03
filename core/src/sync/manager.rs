use crate::{
	prisma::{
		file_path, location, node, object, owned_operation, shared_operation, tag, PrismaClient,
	},
	sync,
};
use prisma_client_rust::ModelTypes;
use sd_sync::*;

use futures::future::join_all;
use serde_json::{from_value, json, to_vec, Value};
use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uhlc::{HLCBuilder, HLC, NTP64};
use uuid::Uuid;

pub struct SyncManager {
	db: Arc<PrismaClient>,
	node: Uuid,
	_clocks: HashMap<Uuid, NTP64>,
	clock: HLC,
	tx: Sender<CRDTOperation>,
}

impl SyncManager {
	pub fn new(db: &Arc<PrismaClient>, node: Uuid) -> (Self, Receiver<CRDTOperation>) {
		let (tx, rx) = mpsc::channel(64);

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
		(ops, queries): (Vec<CRDTOperation>, I),
	) -> prisma_client_rust::Result<<I as prisma_client_rust::BatchItemParent>::ReturnValue> {
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

		self.tx.send(op).await.ok();

		Ok(ret)
	}

	pub async fn get_ops(&self) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		let db = &self.db;

		let owned = db
			.owned_operation()
			.find_many(vec![])
			.include(owned_operation::include!({ node }))
			.exec()
			.await?
			.into_iter()
			.flat_map(|op| {
				Some(CRDTOperation {
					id: Uuid::from_slice(&op.id).ok()?,
					node: Uuid::from_slice(&op.node.pub_id).ok()?,
					timestamp: NTP64(op.timestamp as u64),
					typ: CRDTOperationType::Owned(OwnedOperation {
						model: op.model,
						items: serde_json::from_slice(&op.data).ok()?,
					}),
				})
			});

		let shared = db
			.shared_operation()
			.find_many(vec![])
			.include(shared_operation::include!({ node }))
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
			});

		let mut result: Vec<CRDTOperation> = owned.chain(shared).collect();

		result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

		Ok(result)
	}

	pub async fn ingest_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		let db = &self.db;

		match op.typ {
			CRDTOperationType::Owned(owned_op) => match owned_op.model.as_str() {
				file_path::Types::MODEL => {
					for item in owned_op.items {
						let id: sync::file_path::SyncId = serde_json::from_value(item.id).unwrap();

						let location = db
							.location()
							.find_unique(location::pub_id::equals(id.location.pub_id))
							.select(location::select!({ id }))
							.exec()
							.await?
							.unwrap();

						match item.data {
							OwnedOperationData::Create(mut data) => {
								db.file_path()
									.create(
										id.id,
										location::id::equals(location.id),
										serde_json::from_value(
											data.remove("materialized_path").unwrap(),
										)
										.unwrap(),
										serde_json::from_value(data.remove("name").unwrap())
											.unwrap(),
										serde_json::from_value(
											data.remove("extension").unwrap_or_else(|| {
												serde_json::Value::String("".to_string())
											}),
										)
										.unwrap(),
										data.into_iter()
											.flat_map(|(k, v)| {
												file_path::SetParam::deserialize(&k, v)
											})
											.collect(),
									)
									.exec()
									.await?;
							}
							OwnedOperationData::CreateMany {
								values,
								skip_duplicates,
							} => {
								let location_ids = values
									.iter()
									.map(|(id, _)| {
										serde_json::from_value::<sync::file_path::SyncId>(
											id.clone(),
										)
										.unwrap()
										.location
										.pub_id
									})
									.collect::<HashSet<_>>();

								let location_id_mappings =
									join_all(location_ids.iter().map(|id| async move {
										db.location()
											.find_unique(location::pub_id::equals(id.clone()))
											.exec()
											.await
											.map(|o| o.map(|v| (id, v.id)))
									}))
									.await
									.into_iter()
									.flatten()
									.flatten()
									.collect::<HashMap<_, _>>();

								let mut q = db.file_path().create_many(
									values
										.into_iter()
										.map(|(id, mut data)| {
											let id: sync::file_path::SyncId =
												serde_json::from_value(id).unwrap();

											file_path::create_unchecked(
												id.id,
												*location_id_mappings
													.get(&id.location.pub_id)
													.unwrap(),
												serde_json::from_value(
													data.remove("materialized_path").unwrap(),
												)
												.unwrap(),
												serde_json::from_value(
													data.remove("name").unwrap(),
												)
												.unwrap(),
												serde_json::from_value(
													data.remove("extension").unwrap_or_else(|| {
														serde_json::Value::String("".to_string())
													}),
												)
												.unwrap(),
												data.into_iter()
													.flat_map(|(k, v)| {
														file_path::SetParam::deserialize(&k, v)
													})
													.collect(),
											)
										})
										.collect(),
								);

								if skip_duplicates {
									q = q.skip_duplicates()
								}

								q.exec().await?;
							}
							OwnedOperationData::Update(data) => {
								self.db
									.file_path()
									.update(
										file_path::location_id_id(location.id, id.id),
										data.into_iter()
											.flat_map(|(k, v)| {
												file_path::SetParam::deserialize(&k, v)
											})
											.collect(),
									)
									.exec()
									.await?;
							}
							_ => todo!(),
						}
					}
				}
				location::Types::MODEL => {
					for item in owned_op.items {
						let id: sync::location::SyncId = from_value(item.id).unwrap();

						match item.data {
							OwnedOperationData::Create(mut data) => {
								db.location()
									.create(
										id.pub_id,
										serde_json::from_value(data.remove("name").unwrap())
											.unwrap(),
										serde_json::from_value(data.remove("path").unwrap())
											.unwrap(),
										{
											let val: std::collections::HashMap<String, Value> =
												from_value(data.remove("node").unwrap()).unwrap();
											let val = val.into_iter().next().unwrap();

											node::UniqueWhereParam::deserialize(&val.0, val.1)
												.unwrap()
										},
										data.into_iter()
											.flat_map(|(k, v)| {
												location::SetParam::deserialize(&k, v)
											})
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
				object::Types::MODEL => {
					let id: sync::object::SyncId = from_value(shared_op.record_id).unwrap();

					match shared_op.data {
						SharedOperationData::Create(_) => {
							db.object()
								.upsert(
									object::pub_id::equals(id.pub_id.clone()),
									(id.pub_id, vec![]),
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
					}
				}
				tag::Types::MODEL => {
					let sync::tag::SyncId { pub_id } = from_value(shared_op.record_id).unwrap();

					match shared_op.data {
						SharedOperationData::Create(create_data) => match create_data {
							SharedOperationCreateData::Unique(create_data) => {
								db.tag()
									.create(
										pub_id,
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
									tag::pub_id::equals(pub_id),
									vec![tag::SetParam::deserialize(&field, value).unwrap()],
								)
								.exec()
								.await?;
						}
						SharedOperationData::Delete => {
							db.tag().delete(tag::pub_id::equals(pub_id)).exec().await?;
						}
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
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: [(&'static str, Value); SIZE],
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
