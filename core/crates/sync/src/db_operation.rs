use sd_prisma::prisma::*;
use sd_sync::*;
use uhlc::NTP64;
use uuid::Uuid;

shared_operation::include!(shared_include {
	instance: select { pub_id }
});
relation_operation::include!(relation_include {
	instance: select { pub_id }
});

pub enum DbOperation {
	Shared(shared_include::Data),
	Relation(relation_include::Data),
}

impl DbOperation {
	pub fn timestamp(&self) -> NTP64 {
		NTP64(match self {
			Self::Shared(op) => op.timestamp,
			Self::Relation(op) => op.timestamp,
		} as u64)
	}

	pub fn id(&self) -> Uuid {
		Uuid::from_slice(match self {
			Self::Shared(op) => &op.id,
			Self::Relation(op) => &op.id,
		})
		.unwrap()
	}

	pub fn instance(&self) -> Uuid {
		Uuid::from_slice(match self {
			Self::Shared(op) => &op.instance.pub_id,
			Self::Relation(op) => &op.instance.pub_id,
		})
		.unwrap()
	}

	pub fn into_operation(self) -> CRDTOperation {
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
