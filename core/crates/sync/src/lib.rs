#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

pub mod ingest;

use sd_prisma::{prisma::*, prisma_sync::ModelSyncData};
use sd_sync::*;
use sd_utils::uuid_to_bytes;

use std::{
	collections::{BTreeMap, HashMap},
	fmt,
	sync::Arc,
};

use serde_json::to_vec;
use tokio::sync::{
	broadcast::{self},
	mpsc, RwLock,
};
use uhlc::{HLCBuilder, Timestamp, HLC};
use uuid::Uuid;

pub use sd_prisma::prisma_sync;
pub use uhlc::NTP64;

#[derive(Clone)]
pub enum SyncMessage {
	Ingested,
	Created,
}

pub type Timestamps = Arc<RwLock<HashMap<Uuid, NTP64>>>;

pub struct SyncManager {
	db: Arc<PrismaClient>,
	pub instance: Uuid,
	// TODO: Remove `Mutex` and store this on `ingest` actor
	timestamps: Timestamps,
	clock: HLC,
	pub tx: broadcast::Sender<SyncMessage>,
	pub ingest: ingest::Actor,
}

impl fmt::Debug for SyncManager {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SyncManager").finish()
	}
}

pub struct SyncManagerNew {
	pub manager: SyncManager,
	pub rx: broadcast::Receiver<SyncMessage>,
	pub ingest_rx: mpsc::Receiver<ingest::Request>,
}

impl SyncManager {
	#[allow(clippy::new_ret_no_self)]
	pub fn new(db: &Arc<PrismaClient>, instance: Uuid) -> SyncManagerNew {
		let (tx, rx) = broadcast::channel(64);

		let timestamps: Timestamps = Default::default();

		let (ingest, ingest_rx) = ingest::Actor::spawn(timestamps.clone());

		SyncManagerNew {
			manager: Self {
				db: db.clone(),
				instance,
				clock: HLCBuilder::new().with_id(instance.into()).build(),
				timestamps,
				tx,
				ingest,
			},
			rx,
			ingest_rx,
		}
	}

	pub async fn write_ops<'item, I: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		(_ops, queries): (Vec<CRDTOperation>, I),
	) -> prisma_client_rust::Result<<I as prisma_client_rust::BatchItemParent>::ReturnValue> {
		#[cfg(feature = "emit-messages")]
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

			self.tx.send(SyncMessage::Created).ok();

			res
		};
		#[cfg(not(feature = "emit-messages"))]
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
		#[cfg(feature = "emit-messages")]
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

			self.tx.send(SyncMessage::Created).ok();

			ret
		};
		#[cfg(not(feature = "emit-messages"))]
		let ret = tx._batch(vec![query]).await?.remove(0);

		Ok(ret)
	}

	pub async fn get_ops(
		&self,
		args: GetOpsArgs,
	) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		let Self { db, .. } = self;

		shared_operation::include!(shared_include {
			instance: select { pub_id }
		});
		relation_operation::include!(relation_include {
			instance: select { pub_id }
		});

		enum DbOperation {
			Shared(shared_include::Data),
			Relation(relation_include::Data),
		}

		impl DbOperation {
			fn timestamp(&self) -> NTP64 {
				NTP64(match self {
					Self::Shared(op) => op.timestamp,
					Self::Relation(op) => op.timestamp,
				} as u64)
			}

			fn id(&self) -> Uuid {
				Uuid::from_slice(match self {
					Self::Shared(op) => &op.id,
					Self::Relation(op) => &op.id,
				})
				.unwrap()
			}

			fn instance(&self) -> Uuid {
				Uuid::from_slice(match self {
					Self::Shared(op) => &op.instance.pub_id,
					Self::Relation(op) => &op.instance.pub_id,
				})
				.unwrap()
			}

			fn into_operation(self) -> CRDTOperation {
				CRDTOperation {
					id: self.id(),
					instance: self.instance(),
					timestamp: self.timestamp(),
					typ: match self {
						Self::Shared(op) => CRDTOperationType::Shared(SharedOperation {
							record_id: serde_json::from_slice(&op.record_id).unwrap(),
							model: op.model,
							data: serde_json::from_slice(&op.data).unwrap(),
						}),
						Self::Relation(op) => CRDTOperationType::Relation(RelationOperation {
							relation: op.relation,
							data: serde_json::from_slice(&op.data).unwrap(),
							relation_item: serde_json::from_slice(&op.item_id).unwrap(),
							relation_group: serde_json::from_slice(&op.group_id).unwrap(),
						}),
					},
				}
			}
		}

		macro_rules! db_args {
			($op:ident) => {
				vec![prisma_client_rust::operator::or(
					args.clocks
						.iter()
						.map(|(instance_id, timestamp)| {
							prisma_client_rust::and![
								$op::instance::is(vec![instance::pub_id::equals(uuid_to_bytes(
									*instance_id
								))]),
								$op::timestamp::gte(timestamp.as_u64() as i64)
							]
						})
						.collect(),
				)]
			};
		}

		let (shared, relation) = db
			._batch((
				db.shared_operation()
					.find_many(db_args!(shared_operation))
					.take(args.count as i64)
					.include(shared_include::include()),
				db.relation_operation()
					.find_many(db_args!(relation_operation))
					.take(args.count as i64)
					.include(relation_include::include()),
			))
			.await?;

		let mut ops = BTreeMap::new();

		ops.extend(
			shared
				.into_iter()
				.map(DbOperation::Shared)
				.map(|op| (op.timestamp(), op)),
		);
		ops.extend(
			relation
				.into_iter()
				.map(DbOperation::Relation)
				.map(|op| (op.timestamp(), op)),
		);

		Ok(ops
			.into_values()
			.rev()
			.take(args.count as usize)
			.map(DbOperation::into_operation)
			.collect())
	}

	pub async fn apply_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
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

		self.tx.send(SyncMessage::Ingested).ok();

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

		old_timestamp
			.map(|old| old != op.timestamp.as_u64() as i64)
			.unwrap_or_default()
	}

	pub async fn receive_crdt_operation(&self, op: CRDTOperation) {
		self.clock
			.update_with_timestamp(&Timestamp::new(op.timestamp, op.instance.into()))
			.ok();

		let mut clocks = self.timestamps.write().await;
		let timestamp = clocks.entry(op.instance).or_insert_with(|| op.timestamp);

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
				vec![instance::timestamp::set(Some(op_timestamp.as_u64() as i64))],
			)
			.exec()
			.await
			.ok();
	}

	pub async fn register_instance(&self, instance_id: Uuid) {
		self.timestamps.write().await.insert(instance_id, NTP64(0));
	}
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct GetOpsArgs {
	pub clocks: Vec<(Uuid, NTP64)>,
	pub count: u32,
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
