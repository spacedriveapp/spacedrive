use crate::{
	CRDTOperation, CRDTOperationData, DevicePubId, RelationSyncId, RelationSyncModel,
	SharedSyncModel, SyncId, SyncModel,
};

use uhlc::HLC;

macro_rules! msgpack {
	(nil) => {
		::rmpv::Value::Nil
	};
	($e:expr) => {{
		let bytes = rmp_serde::to_vec_named(&$e).expect("failed to serialize msgpack");
		let value: rmpv::Value = rmp_serde::from_slice(&bytes).expect("failed to deserialize msgpack");

		value
	}}
}

pub trait OperationFactory {
	fn get_clock(&self) -> &HLC;
	fn get_device_pub_id(&self) -> DevicePubId;

	fn new_op<SId: SyncId<Model: SyncModel>>(
		&self,
		id: &SId,
		data: CRDTOperationData,
	) -> CRDTOperation {
		let timestamp = self.get_clock().new_timestamp();

		CRDTOperation {
			device_pub_id: self.get_device_pub_id(),
			timestamp: *timestamp.get_time(),
			model_id: <SId::Model as SyncModel>::MODEL_ID,
			record_id: msgpack!(id),
			data,
		}
	}

	fn shared_create(
		&self,
		id: impl SyncId<Model = impl SharedSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> Vec<CRDTOperation> {
		vec![self.new_op(
			&id,
			CRDTOperationData::Create(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		)]
	}

	fn shared_update(
		&self,
		id: impl SyncId<Model = impl SharedSyncModel>,
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

	fn shared_delete(&self, id: impl SyncId<Model = impl SharedSyncModel>) -> CRDTOperation {
		self.new_op(&id, CRDTOperationData::Delete)
	}

	fn relation_create(
		&self,
		id: impl RelationSyncId<Model = impl RelationSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> Vec<CRDTOperation> {
		vec![self.new_op(
			&id,
			CRDTOperationData::Create(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		)]
	}
	fn relation_update(
		&self,
		id: impl RelationSyncId<Model = impl RelationSyncModel>,
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
	fn relation_delete(
		&self,
		id: impl RelationSyncId<Model = impl RelationSyncModel>,
	) -> CRDTOperation {
		self.new_op(&id, CRDTOperationData::Delete)
	}
}

#[macro_export]
macro_rules! sync_entry {
    ($value:expr, $($prisma_column_module:tt)+) => {
        ($($prisma_column_module)+::NAME, ::sd_utils::msgpack!($value))
    }
}

#[macro_export]
macro_rules! option_sync_entry {
    ($value:expr, $($prisma_column_module:tt)+) => {
        $value.map(|value| $crate::sync_entry!(value, $($prisma_column_module)+))
    }
}

#[macro_export]
macro_rules! sync_db_entry {
    ($value:expr, $($prisma_column_module:tt)+) => {{
        let value = $value.into();
        (
			$crate::sync_entry!(&value, $($prisma_column_module)+),
			$($prisma_column_module)+::set(Some(value))
		)
    }}
}

#[macro_export]
macro_rules! option_sync_db_entry {
	($value:expr, $($prisma_column_module:tt)+) => {
	   $value.map(|value| $crate::sync_db_entry!(value, $($prisma_column_module)+))
	};
}
