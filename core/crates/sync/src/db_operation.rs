use sd_prisma::prisma::{cloud_crdt_operation, crdt_operation};
use sd_sync::CRDTOperation;
use uhlc::NTP64;
use uuid::Uuid;

crdt_operation::include!(crdt_include {
	instance: select { pub_id }
});

cloud_crdt_operation::include!(cloud_crdt_include {
	instance: select { pub_id }
});

impl crdt_include::Data {
	pub fn timestamp(&self) -> NTP64 {
		NTP64(self.timestamp as u64)
	}

	pub fn id(&self) -> Uuid {
		Uuid::from_slice(&self.id).unwrap()
	}

	pub fn instance(&self) -> Uuid {
		Uuid::from_slice(&self.instance.pub_id).unwrap()
	}

	pub fn into_operation(self) -> CRDTOperation {
		CRDTOperation {
			id: self.id(),
			instance: self.instance(),
			timestamp: self.timestamp(),
			record_id: serde_json::from_slice(&self.record_id).unwrap(),
			model: self.model,
			data: serde_json::from_slice(&self.data).unwrap(),
			// match self {
			// 	Self::Shared(op) => CRDTOperationType::Shared(SharedOperation {
			// 		record_id: serde_json::from_slice(&op.record_id).unwrap(),
			// 		model: op.model,
			// 		data: serde_json::from_slice(&op.data).unwrap(),
			// 	}),
			// 	Self::Relation(op) => CRDTOperationType::Relation(RelationOperation {
			// 		relation: op.relation,
			// 		data: serde_json::from_slice(&op.data).unwrap(),
			// 		relation_item: serde_json::from_slice(&op.item_id).unwrap(),
			// 		relation_group: serde_json::from_slice(&op.group_id).unwrap(),
			// 	}),
			// },
		}
	}
}

impl cloud_crdt_include::Data {
	pub fn timestamp(&self) -> NTP64 {
		NTP64(self.timestamp as u64)
	}

	pub fn id(&self) -> Uuid {
		Uuid::from_slice(&self.id).unwrap()
	}

	pub fn instance(&self) -> Uuid {
		Uuid::from_slice(&self.instance.pub_id).unwrap()
	}

	pub fn into_operation(self) -> CRDTOperation {
		CRDTOperation {
			id: self.id(),
			instance: self.instance(),
			timestamp: self.timestamp(),
			record_id: serde_json::from_slice(&self.record_id).unwrap(),
			model: self.model,
			data: serde_json::from_slice(&self.data).unwrap(),
			// match self {
			// 	Self::Shared(op) => ,
			// 	Self::Relation(op) => CRDTOperationType::Relation(RelationOperation {
			// 		relation: op.relation,
			// 		data: serde_json::from_slice(&op.data).unwrap(),
			// 		relation_item: serde_json::from_slice(&op.item_id).unwrap(),
			// 		relation_group: serde_json::from_slice(&op.group_id).unwrap(),
			// 	}),
			// },
		}
	}
}
