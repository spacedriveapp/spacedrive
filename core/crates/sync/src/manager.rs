use sd_prisma::prisma::*;
use sd_sync::*;
use sd_utils::uuid_to_bytes;

use crate::{db_operation::*, *};
use std::{cmp::Ordering, ops::Deref, sync::Arc};
use tokio::sync::{broadcast, mpsc};
use uhlc::{HLCBuilder, HLC};
use uuid::Uuid;

pub struct Manager {
	pub tx: broadcast::Sender<SyncMessage>,
	pub ingest: ingest::Handler,
	shared: Arc<SharedState>,
}

pub struct SyncManagerNew {
	pub manager: Manager,
	pub rx: broadcast::Receiver<SyncMessage>,
	pub ingest_rx: mpsc::Receiver<ingest::Request>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct GetOpsArgs {
	pub clocks: Vec<(Uuid, NTP64)>,
	pub count: u32,
}

impl Manager {
	pub fn new(db: &Arc<PrismaClient>, instance: Uuid) -> SyncManagerNew {
		let (tx, rx) = broadcast::channel(64);

		let timestamps: Timestamps = Default::default();
		let clock = HLCBuilder::new().with_id(instance.into()).build();

		let shared = Arc::new(SharedState {
			db: db.clone(),
			instance,
			timestamps,
			clock,
		});

		let (ingest, ingest_rx) = ingest::Actor::spawn(shared.clone());

		SyncManagerNew {
			manager: Self { shared, tx, ingest },
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
		let db = &self.db;

		macro_rules! db_args {
			($args:ident, $op:ident) => {
				vec![prisma_client_rust::operator::or(
					$args
						.clocks
						.iter()
						.map(|(instance_id, timestamp)| {
							prisma_client_rust::and![
								$op::instance::is(vec![instance::pub_id::equals(uuid_to_bytes(
									*instance_id
								))]),
								$op::timestamp::gt(timestamp.as_u64() as i64)
							]
						})
						.chain([
							$op::instance::is_not(vec![
								instance::pub_id::in_vec(
									$args
										.clocks
										.iter()
										.map(|(instance_id, _)| {
											uuid_to_bytes(*instance_id)
										})
										.collect()
								)
							])
						])
						.collect(),
				)]
			};
		}

		let (shared, relation) = db
			._batch((
				db.shared_operation()
					.find_many(db_args!(args, shared_operation))
					.take(args.count as i64)
					.order_by(shared_operation::timestamp::order(SortOrder::Asc))
					.include(shared_include::include()),
				db.relation_operation()
					.find_many(db_args!(args, relation_operation))
					.take(args.count as i64)
					.order_by(relation_operation::timestamp::order(SortOrder::Asc))
					.include(relation_include::include()),
			))
			.await?;

		let mut ops: Vec<_> = []
			.into_iter()
			.chain(shared.into_iter().map(DbOperation::Shared))
			.chain(relation.into_iter().map(DbOperation::Relation))
			.collect();

		ops.sort_by(|a, b| match a.timestamp().cmp(&b.timestamp()) {
			Ordering::Equal => a.instance().cmp(&b.instance()),
			o => o,
		});

		Ok(ops
			.into_iter()
			.take(args.count as usize)
			.map(DbOperation::into_operation)
			.collect())
	}
}

impl OperationFactory for Manager {
	fn get_clock(&self) -> &HLC {
		&self.clock
	}

	fn get_instance(&self) -> Uuid {
		self.instance
	}
}

impl Deref for Manager {
	type Target = SharedState;

	fn deref(&self) -> &Self::Target {
		&self.shared
	}
}
