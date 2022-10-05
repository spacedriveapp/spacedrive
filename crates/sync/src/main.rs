use serde_json::*;

use sd_sync::*;

fn map<const N: usize>(arr: [(&str, Value); N]) -> Map<String, Value> {
	arr.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn main() {
	let uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");

	let mut db = Db::new(uuid);

	db.receive_crdt_operations(vec![db.create_crdt_operation(CRDTOperationType::Owned(
		OwnedOperation {
			model: "FilePath".to_string(),
			items: vec![OwnedOperationItem {
				id: json!(0),
				data: OwnedOperationData::Create(map([("path", json!("some/file.path"))])),
			}],
		},
	))]);

	dbg!(&db);

	db.receive_crdt_operations(vec![db.create_crdt_operation(CRDTOperationType::Shared(
		SharedOperation {
			record_id: json!(0),
			model: "File".to_string(),
			data: SharedOperationData::Create(SharedOperationCreateData::Atomic),
		},
	))]);

	dbg!(&db);

	db.receive_crdt_operations(vec![db.create_crdt_operation(CRDTOperationType::Shared(
		SharedOperation {
			record_id: json!(0),
			model: "File".to_string(),
			data: SharedOperationData::Update {
				field: "name".to_string(),
				value: json!("Lmaoooo"),
			},
		},
	))]);

	dbg!(&db);

	db.receive_crdt_operations(vec![db.create_crdt_operation(CRDTOperationType::Owned(
		OwnedOperation {
			model: "FilePath".to_string(),
			items: vec![OwnedOperationItem {
				id: json!(0),
				data: OwnedOperationData::Update(map([("file", json!(0))])),
			}],
		},
	))]);

	dbg!(&db);
}
