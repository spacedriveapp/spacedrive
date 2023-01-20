mod crdt;
// mod db;

pub use crdt::*;
// pub use db::*;

use prisma_client_rust::ModelTypes;
use serde::{de::DeserializeOwned, Serialize};
use serde_value::Value;
use std::collections::BTreeMap;

pub trait SyncId: Serialize + DeserializeOwned {
	type ModelTypes: SyncType;
}

pub trait SyncType: ModelTypes {
	type SyncId: SyncId;
	type Marker: SyncTypeMarker;
}

pub trait SyncTypeMarker {}

pub struct LocalSyncType;
impl SyncTypeMarker for LocalSyncType {}

pub struct OwnedSyncType;
impl SyncTypeMarker for OwnedSyncType {}

pub struct SharedSyncType;
impl SyncTypeMarker for SharedSyncType {}

pub struct RelationSyncType;
impl SyncTypeMarker for RelationSyncType {}

pub trait CreateCRDTMutation<T: ModelTypes> {
	fn operation_from_data(
		d: &BTreeMap<String, Value>,
		typ: CreateOperationType,
	) -> CRDTOperationType;
}

pub enum CreateOperationType {
	Owned,
	SharedUnique,
	SharedAtomic,
	Relation,
}

impl<T: ModelTypes> CreateCRDTMutation<T> for prisma_client_rust::Create<'_, T> {
	fn operation_from_data(
		_: &BTreeMap<String, Value>,
		typ: CreateOperationType,
	) -> CRDTOperationType {
		match typ {
			CreateOperationType::Owned => {
				todo!()
				// let id = serde_json::to_value(
				// 	d.iter()
				// 		.filter(|(field, _)| T::id_fields().iter().any(|f| f == field))
				// 		.collect::<BTreeMap<_, _>>(),
				// )
				// .unwrap();

				// CRDTOperationType::Owned(OwnedOperation {
				// 	model: T::MODEL.to_string(),
				// 	items: [OwnedOperationItem {
				// 		id,
				// 		data: OwnedOperationData::Create(
				// 			d.clone()
				// 				.into_iter()
				// 				.filter(|(field, _)| T::id_fields().iter().all(|f| f != field))
				// 				.map(|(k, v)| (k, serde_json::to_value(v).unwrap()))
				// 				.collect(),
				// 		),
				// 	}]
				// 	.to_vec(),
				// })
			}
			_ => todo!(),
		}
	}
}
