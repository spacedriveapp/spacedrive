use serde_json::{json, Value};
use uhlc::HLC;
use uuid::Uuid;

use crate::{CRDTOperation, CRDTOperationType, RelationOperation, RelationOperationData, RelationSyncId, RelationSyncModel, SharedOperation, SharedOperationData, SharedSyncModel, SyncId};

pub trait OperationFactory {
	fn get_clock(&self) -> &HLC;
	fn get_instance(&self) -> Uuid;

	fn new_op(&self, typ: CRDTOperationType) -> CRDTOperation {
		let timestamp = self.get_clock().new_timestamp();

		CRDTOperation {
			instance: self.get_instance(),
			timestamp: *timestamp.get_time(),
			id: Uuid::new_v4(),
			typ,
		}
	}

	fn shared_op<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: &TSyncId,
		data: SharedOperationData,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data,
		}))
	}

	fn shared_create<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, Value)> + 'static,
	) -> Vec<CRDTOperation> {
		[self.shared_op(&id, SharedOperationData::Create)]
			.into_iter()
			.chain(values.into_iter().map(|(name, value)| {
				self.shared_op(
					&id,
					SharedOperationData::Update {
						field: name.to_string(),
						value,
					},
				)
			}))
			.collect()
	}
	fn shared_update<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: TSyncId,
		field: impl Into<String>,
		value: Value,
	) -> CRDTOperation {
		self.shared_op(
			&id,
			SharedOperationData::Update {
				field: field.into(),
				value,
			},
		)
	}
	fn shared_delete<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.shared_op(&id, SharedOperationData::Delete)
	}

	fn relation_op<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: &TSyncId,
		data: RelationOperationData,
	) -> CRDTOperation {
		let (item_id, group_id) = id.split();

		self.new_op(CRDTOperationType::Relation(RelationOperation {
			relation_item: json!(item_id),
			relation_group: json!(group_id),
			relation: TModel::MODEL.to_string(),
			data,
		}))
	}

	fn relation_create<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, Value)> + 'static,
	) -> Vec<CRDTOperation> {
		[self.relation_op(&id, RelationOperationData::Create)]
			.into_iter()
			.chain(values.into_iter().map(|(name, value)| {
				self.relation_op(
					&id,
					RelationOperationData::Update {
						field: name.to_string(),
						value,
					},
				)
			}))
			.collect()
	}
	fn relation_update<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: TSyncId,
		field: impl Into<String>,
		value: Value,
	) -> CRDTOperation {
		self.relation_op(
			&id,
			RelationOperationData::Update {
				field: field.into(),
				value,
			},
		)
	}
	fn relation_delete<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.relation_op(&id, RelationOperationData::Delete)
	}
}
