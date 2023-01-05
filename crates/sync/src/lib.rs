mod crdt;
// mod db;

pub use crdt::*;
// pub use db::*;

use prisma_client_rust::ModelActions;
use serde_value::Value;
use std::collections::BTreeMap;

pub trait CreateCRDTMutation<T: ModelActions> {
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

impl<T: ModelActions> CreateCRDTMutation<T> for prisma_client_rust::Create<'_, T> {
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
