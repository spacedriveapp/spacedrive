use std::{collections::HashMap, time::Duration};

use rspc::Type;
use serde::*;
use serde_json::*;
use uhlc::{HLCBuilder, Timestamp, HLC, NTP64};
use uuid::Uuid;

use super::crdt::*;

// Bytes
#[derive(Default, Debug, Serialize, Type, Clone)]
pub struct Color {
	pub red: u8,
	pub green: u8,
	pub blue: u8,
}

// Unique Shared
#[derive(Default, Debug, Serialize, Type, Clone)]
pub struct Tag {
	pub color: Color,
	pub name: String,
}

#[derive(Default, Debug, Serialize, Type, Clone)]
pub struct TagOnObject {
	pub tag_id: Uuid,
	pub object_id: Uuid,
}

// Atomic Shared
#[derive(Default, Debug, Serialize, Type, Clone)]
pub struct Object {
	pub id: Uuid,
	pub name: String,
}

// Owned
#[derive(Serialize, Deserialize, Debug, Type, Clone)]
pub struct FilePath {
	pub id: Uuid,
	pub path: String,
	pub file: Option<Uuid>,
}

pub struct Db {
	pub objects: HashMap<Uuid, Object>,
	pub file_paths: HashMap<Uuid, FilePath>,
	pub tags: HashMap<Uuid, Tag>,
	pub tags_on_objects: HashMap<(Uuid, Uuid), TagOnObject>,
	pub _operations: Vec<CRDTOperation>,
	pub _clocks: HashMap<Uuid, NTP64>,
	_clock: HLC,
	_node: Uuid,
}

impl std::fmt::Debug for Db {
	fn fmt(&self, f: &mut __private::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Db")
			.field("files", &self.objects)
			.field("file_paths", &self.file_paths)
			.finish()
	}
}

impl Db {
	pub fn new(node: Uuid) -> Self {
		Self {
			objects: Default::default(),
			file_paths: Default::default(),
			tags: Default::default(),
			tags_on_objects: Default::default(),
			_clocks: Default::default(),
			_node: node,
			_clock: HLCBuilder::new().with_id(node.into()).build(),
			_operations: Default::default(),
		}
	}

	pub fn register_node(&mut self, id: Uuid) {
		self._clocks
			.entry(id)
			.or_insert_with(|| Duration::from_millis(0).into());
	}

	pub fn create_crdt_operation(&mut self, typ: CRDTOperationType) -> CRDTOperation {
		let hlc_timestamp = self._clock.new_timestamp();

		CRDTOperation {
			node: self._node,
			timestamp: *hlc_timestamp.get_time(),
			id: Uuid::new_v4(),
			typ,
		}
	}

	fn compare_messages(&self, operations: Vec<CRDTOperation>) -> Vec<(CRDTOperation, bool)> {
		operations
			.into_iter()
			.map(|op| (op.id, op))
			.collect::<HashMap<_, _>>()
			.into_iter()
			.filter_map(|(_, op)| {
				match &op.typ {
					CRDTOperationType::Owned(_) => {
						self._operations.iter().find(|find_op| match &find_op.typ {
							CRDTOperationType::Owned(_) => {
								find_op.timestamp >= op.timestamp && find_op.node == op.node
							}
							_ => false,
						})
					}
					CRDTOperationType::Shared(shared_op) => {
						self._operations.iter().find(|find_op| match &find_op.typ {
							CRDTOperationType::Shared(find_shared_op) => {
								shared_op.model == find_shared_op.model
									&& shared_op.record_id == find_shared_op.record_id
									&& find_op.timestamp >= op.timestamp
							}
							_ => false,
						})
					}
					CRDTOperationType::Relation(relation_op) => {
						self._operations.iter().find(|find_op| match &find_op.typ {
							CRDTOperationType::Relation(find_relation_op) => {
								relation_op.relation == find_relation_op.relation
									&& relation_op.relation_item == find_relation_op.relation_item
									&& relation_op.relation_group == find_relation_op.relation_group
							}
							_ => false,
						})
					}
				}
				.map(|old_op| (old_op.timestamp != op.timestamp).then_some(true))
				.unwrap_or(Some(false))
				.map(|old| (op, old))
			})
			.collect()
	}

	pub fn receive_crdt_operations(&mut self, ops: Vec<CRDTOperation>) {
		for op in &ops {
			self._clock
				.update_with_timestamp(&Timestamp::new(op.timestamp, op.node.into()))
				.ok();

			self._clocks.insert(op.node, op.timestamp);
		}

		for (op, old) in self.compare_messages(ops) {
			let push_op = op.clone();

			if !old {
				match op.typ {
					CRDTOperationType::Shared(shared_op) => match shared_op.model.as_str() {
						"Object" => {
							let id = shared_op.record_id;

							match shared_op.data {
								SharedOperationData::Create(SharedOperationCreateData::Atomic) => {
									self.objects.insert(
										id,
										Object {
											id,
											..Default::default()
										},
									);
								}
								SharedOperationData::Update { field, value } => {
									let mut file = self.objects.get_mut(&id).unwrap();

									match field.as_str() {
										"name" => {
											file.name = from_value(value).unwrap();
										}
										_ => unreachable!(),
									}
								}
								SharedOperationData::Delete => {
									self.objects.remove(&id).unwrap();
								}
								_ => {}
							}
						}
						_ => unreachable!(),
					},
					CRDTOperationType::Owned(owned_op) => match owned_op.model.as_str() {
						"FilePath" => {
							for item in owned_op.items {
								let id = from_value(item.id).unwrap();

								match item.data {
									OwnedOperationData::Create(data) => {
										self.file_paths
											.insert(id, from_value(Value::Object(data)).unwrap());
									}
									OwnedOperationData::Update(data) => {
										let obj = self.file_paths.get_mut(&id).unwrap();

										for (key, value) in data {
											match key.as_str() {
												"path" => obj.path = from_value(value).unwrap(),
												"file" => obj.file = from_value(value).unwrap(),
												_ => unreachable!(),
											}
										}
									}
									OwnedOperationData::Delete => {
										self.file_paths.remove(&id);
									}
								}
							}
						}
						_ => unreachable!(),
					},
					CRDTOperationType::Relation(relation_op) => match relation_op.relation.as_str()
					{
						"TagOnObject" => match relation_op.data {
							RelationOperationData::Create => {
								self.tags_on_objects.insert(
									(relation_op.relation_item, relation_op.relation_group),
									TagOnObject {
										object_id: relation_op.relation_item,
										tag_id: relation_op.relation_group,
									},
								);
							}
							RelationOperationData::Update { field: _, value: _ } => {
								// match field.as_str() {
								// 	_ => unreachable!(),
								// }
							}
							RelationOperationData::Delete => {
								self.tags_on_objects
									.remove(&(
										relation_op.relation_item,
										relation_op.relation_group,
									))
									.unwrap();
							}
						},
						_ => unreachable!(),
					},
				}

				self._operations.push(push_op)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn to_map(v: &impl serde::Serialize) -> serde_json::Map<String, Value> {
		match to_value(&v).unwrap() {
			Value::Object(m) => m,
			_ => unreachable!(),
		}
	}

	#[test]
	fn test() {
		let mut dbs = vec![];

		for _ in 0..3 {
			let id = Uuid::new_v4();

			dbs.push(Db::new(id.clone()));

			let ids = dbs.iter().map(|db| db._node.clone()).collect::<Vec<_>>();

			for db in &mut dbs {
				for id in &ids {
					db.register_node(id.clone());
				}
			}
		}

		for db in &mut dbs {
			let id = Uuid::new_v4();

			let file_path = FilePath {
				id,
				path: String::new(),
				file: None,
			};

			let op = db.create_crdt_operation(CRDTOperationType::Owned(OwnedOperation {
				model: "FilePath".to_string(),
				items: vec![OwnedOperationItem {
					id: serde_json::to_value(id).unwrap(),
					data: OwnedOperationData::Create(to_map(&file_path)),
				}],
			}));

			db.receive_crdt_operations(vec![op]);
		}

		for db in &dbs {
			assert_eq!(db._operations.len(), 1);
		}

		let ops = dbs
			.iter()
			.flat_map(|db| db._operations.clone())
			.collect::<Vec<_>>();

		for db in &mut dbs {
			db.receive_crdt_operations(ops.clone())
		}

		for db in &dbs {
			assert_eq!(db.file_paths.len(), 3);
			assert_eq!(db._operations.len(), 3);
		}

		for _ in 0..2 {
			let db = &mut dbs[0];
			let id = Uuid::new_v4();

			let file_path = FilePath {
				id,
				path: String::new(),
				file: None,
			};

			let op = db.create_crdt_operation(CRDTOperationType::Owned(OwnedOperation {
				model: "FilePath".to_string(),
				items: vec![OwnedOperationItem {
					id: serde_json::to_value(id).unwrap(),
					data: OwnedOperationData::Create(to_map(&file_path)),
				}],
			}));

			db.receive_crdt_operations(vec![op]);
		}

		let ops = dbs
			.iter()
			.flat_map(|db| db._operations.clone())
			.collect::<Vec<_>>();

		for db in &mut dbs {
			db.receive_crdt_operations(ops.clone());
		}

		for db in &dbs {
			assert_eq!(db.file_paths.len(), 5);
			assert_eq!(db._operations.len(), 5);
		}

		for _ in 0..4 {
			let ops = dbs
				.iter()
				.flat_map(|db| db._operations.clone())
				.collect::<Vec<_>>();
			dbs[0].receive_crdt_operations(ops);
		}

		for db in &dbs {
			assert_eq!(db.file_paths.len(), 5);
			assert_eq!(db._operations.len(), 5);
		}
	}
}
