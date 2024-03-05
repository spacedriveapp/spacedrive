use prisma_client_rust::ModelTypes;
use uhlc::HLC;
use uuid::Uuid;

use crate::{
	CRDTOperation, CRDTOperationData, RelationSyncId, RelationSyncModel, SharedSyncModel, SyncId,
};

macro_rules! msgpack {
	($e:expr) => {
		::rmp_serde::to_vec_named(&$e).expect("failed to serialize msgpack")
	}
}

pub trait OperationFactory {
	fn get_clock(&self) -> &HLC;
	fn get_instance(&self) -> Uuid;

	fn new_op<TSyncId: SyncId<Model = TModel>, TModel: ModelTypes>(
		&self,
		id: &TSyncId,
		data: CRDTOperationData,
	) -> CRDTOperation {
		let timestamp = self.get_clock().new_timestamp();

		CRDTOperation {
			instance: self.get_instance(),
			timestamp: *timestamp.get_time(),
			id: Uuid::new_v4(),
			model: TModel::MODEL.to_string(),
			record_id: msgpack!(id),
			data,
		}
	}

	fn shared_create<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> Vec<CRDTOperation> {
		[self.new_op(&id, CRDTOperationData::Create)]
			.into_iter()
			.chain(values.into_iter().map(|(name, value)| {
				self.new_op(
					&id,
					CRDTOperationData::Update {
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
		value: rmpv::Value,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Update {
				field: field.into(),
				value,
			},
		)
	}
	fn shared_delete<TSyncId: SyncId<Model = TModel>, TModel: SharedSyncModel>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.new_op(&id, CRDTOperationData::Delete)
	}

	fn relation_create<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> Vec<CRDTOperation> {
		[self.new_op(&id, CRDTOperationData::Create)]
			.into_iter()
			.chain(values.into_iter().map(|(name, value)| {
				self.new_op(
					&id,
					CRDTOperationData::Update {
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
		value: rmpv::Value,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Update {
				field: field.into(),
				value,
			},
		)
	}
	fn relation_delete<TSyncId: RelationSyncId<Model = TModel>, TModel: RelationSyncModel>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.new_op(&id, CRDTOperationData::Delete)
	}
}

#[macro_export]
macro_rules! sync_entry {
    ($v:expr, $($m:tt)*) => {{
        let v = $v;
        ($($m)*::NAME, ::rmpv::ext::to_value(&v).expect("failed to serialize msgpack"))
    }}
}

#[macro_export]
macro_rules! option_sync_entry {
    ($v:expr, $($m:tt)*) => {
        $v.map(|v| $crate::sync_entry!(v, $($m)*))
    }
}

#[macro_export]
macro_rules! sync_db_entry {
    ($v:expr, $($m:tt)*) => {{
        let v = $v;
        ($crate::sync_entry!(&v, $($m)*), $($m)*::set(Some(v)))
    }}
}

#[macro_export]
macro_rules! option_sync_db_entry {
	($v:expr, $($m:tt)*) => {
	   $v.map(|v| $crate::sync_db_entry!(v, $($m)*))
	};
}
