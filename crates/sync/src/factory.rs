use crate::{
	CRDTOperation, CRDTOperationData, DevicePubId, RelationSyncId, RelationSyncModel,
	SharedSyncModel, SyncId, SyncModel,
};

use uhlc::HLC;

pub trait OperationFactory {
	fn get_clock(&self) -> &HLC;

	fn get_device_pub_id(&self) -> DevicePubId;

	fn new_op<SId: SyncId<Model: SyncModel>>(
		&self,
		id: &SId,
		data: CRDTOperationData,
	) -> CRDTOperation {
		CRDTOperation {
			device_pub_id: self.get_device_pub_id(),
			timestamp: *self.get_clock().new_timestamp().get_time(),
			model_id: <SId::Model as SyncModel>::MODEL_ID,
			record_id: rmp_serde::from_slice::<rmpv::Value>(
				&rmp_serde::to_vec_named(id).expect("failed to serialize record id to msgpack"),
			)
			.expect("failed to deserialize record id to msgpack value"),
			data,
		}
	}

	fn shared_create(
		&self,
		id: impl SyncId<Model = impl SharedSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Create(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		)
	}

	fn shared_update(
		&self,
		id: impl SyncId<Model = impl SharedSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Update(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		)
	}

	fn shared_delete(&self, id: impl SyncId<Model = impl SharedSyncModel>) -> CRDTOperation {
		self.new_op(&id, CRDTOperationData::Delete)
	}

	fn relation_create(
		&self,
		id: impl RelationSyncId<Model = impl RelationSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Create(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
		)
	}

	fn relation_update(
		&self,
		id: impl RelationSyncId<Model = impl RelationSyncModel>,
		values: impl IntoIterator<Item = (&'static str, rmpv::Value)> + 'static,
	) -> CRDTOperation {
		self.new_op(
			&id,
			CRDTOperationData::Update(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			),
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
	(nil, $($prisma_column_module:tt)+) => {
        ($($prisma_column_module)+::NAME, ::sd_utils::msgpack!(nil))
    };

    ($value:expr, $($prisma_column_module:tt)+) => {
        ($($prisma_column_module)+::NAME, ::sd_utils::msgpack!($value))
    };

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
macro_rules! sync_db_nullable_entry {
    ($value:expr, $($prisma_column_module:tt)+) => {{
        let value = $value.into();
        (
			$crate::sync_entry!(&value, $($prisma_column_module)+),
			$($prisma_column_module)+::set(value)
		)
    }}
}

#[macro_export]
macro_rules! sync_db_not_null_entry {
    ($value:expr, $($prisma_column_module:tt)+) => {{
        let value = $value.into();
        (
			$crate::sync_entry!(&value, $($prisma_column_module)+),
			$($prisma_column_module)+::set(value)
		)
    }}
}

#[macro_export]
macro_rules! option_sync_db_entry {
	($value:expr, $($prisma_column_module:tt)+) => {
	   $value.map(|value| $crate::sync_db_entry!(value, $($prisma_column_module)+))
	};
}
