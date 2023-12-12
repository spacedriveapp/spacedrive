mod crdt;
mod factory;
mod model_traits;

pub use crdt::*;
pub use factory::*;
pub use model_traits::*;

pub use uhlc::NTP64;

// fn compare_messages(&self, operations: Vec<CRDTOperation>) -> Vec<(CRDTOperation, bool)> {
// 	operations
// 		.into_iter()
// 		.map(|op| (op.id, op))
// 		.collect::<HashMap<_, _>>()
// 		.into_iter()
// 		.filter_map(|(_, op)| {
// 			match &op.typ {
// 				CRDTOperationType::Owned(_) => {
// 					self._operations.iter().find(|find_op| match &find_op.typ {
// 						CRDTOperationType::Owned(_) => {
// 							find_op.timestamp >= op.timestamp && find_op.node == op.node
// 						}
// 						_ => false,
// 					})
// 				}
// 				CRDTOperationType::Shared(shared_op) => {
// 					self._operations.iter().find(|find_op| match &find_op.typ {
// 						CRDTOperationType::Shared(find_shared_op) => {
// 							shared_op.model == find_shared_op.model
// 							&& shared_op.record_id == find_shared_op.record_id
// 							&& find_op.timestamp >= op.timestamp
// 						}
// 						_ => false,
// 					})
// 				}
// 				CRDTOperationType::Relation(relation_op) => {
// 					self._operations.iter().find(|find_op| match &find_op.typ {
// 						CRDTOperationType::Relation(find_relation_op) => {
// 							relation_op.relation == find_relation_op.relation
// 							&& relation_op.relation_item == find_relation_op.relation_item
// 							&& relation_op.relation_group == find_relation_op.relation_group
// 						}
// 						_ => false,
// 					})
// 				}
// 			}
// 			.map(|old_op| (old_op.timestamp != op.timestamp).then_some(true))
// 			.unwrap_or(Some(false))
// 			.map(|old| (op, old))
// 		})
// 		.collect()
// }

// pub fn receive_crdt_operations(&mut self, ops: Vec<CRDTOperation>) {
// 	for op in &ops {
// 		self._clock
// 			.update_with_timestamp(&Timestamp::new(op.timestamp, op.node.into()))
// 			.ok();

// 		self._clocks.insert(op.node, op.timestamp);
// 	}

// 	for (op, old) in self.compare_messages(ops) {
// 		let push_op = op.clone();

// 		if !old {
// 			match op.typ {
// 				CRDTOperationType::Shared(shared_op) => match shared_op.model.as_str() {
// 					"Object" => {
// 						let id = shared_op.record_id;

// 						match shared_op.data {
// 							SharedOperationData::Create(SharedOperationCreateData::Atomic) => {
// 								self.objects.insert(
// 									id,
// 									Object {
// 										id,
// 										..Default::default()
// 									},
// 								);
// 							}
// 							SharedOperationData::Update { field, value } => {
// 								let mut file = self.objects.get_mut(&id).unwrap();

// 								match field.as_str() {
// 									"name" => {
// 										file.name = from_value(value).unwrap();
// 									}
// 									_ => unreachable!(),
// 								}
// 							}
// 							SharedOperationData::Delete => {
// 								self.objects.remove(&id).unwrap();
// 							}
// 							_ => {}
// 						}
// 					}
// 					_ => unreachable!(),
// 				},
// 				CRDTOperationType::Owned(owned_op) => match owned_op.model.as_str() {
// 					"FilePath" => {
// 						for item in owned_op.items {
// 							let id = from_value(item.id).unwrap();

// 							match item.data {
// 								OwnedOperationData::Create(data) => {
// 									self.file_paths.insert(
// 										id,
// 										from_value(Value::Object(data.into_iter().collect()))
// 											.unwrap(),
// 									);
// 								}
// 								OwnedOperationData::Update(data) => {
// 									let obj = self.file_paths.get_mut(&id).unwrap();

// 									for (key, value) in data {
// 										match key.as_str() {
// 											"path" => obj.path = from_value(value).unwrap(),
// 											"file" => obj.file = from_value(value).unwrap(),
// 											_ => unreachable!(),
// 										}
// 									}
// 								}
// 								OwnedOperationData::Delete => {
// 									self.file_paths.remove(&id);
// 								}
// 							}
// 						}
// 					}
// 					_ => unreachable!(),
// 				},
// 				CRDTOperationType::Relation(relation_op) => match relation_op.relation.as_str()
// 				{
// 					"TagOnObject" => match relation_op.data {
// 						RelationOperationData::Create => {
// 							self.tags_on_objects.insert(
// 								(relation_op.relation_item, relation_op.relation_group),
// 								TagOnObject {
// 									object_id: relation_op.relation_item,
// 									tag_id: relation_op.relation_group,
// 								},
// 							);
// 						}
// 						RelationOperationData::Update { field: _, value: _ } => {
// 							// match field.as_str() {
// 							// 	_ => unreachable!(),
// 							// }
// 						}
// 						RelationOperationData::Delete => {
// 							self.tags_on_objects
// 								.remove(&(
// 									relation_op.relation_item,
// 									relation_op.relation_group,
// 								))
// 								.unwrap();
// 						}
// 					},
// 					_ => unreachable!(),
// 				},
// 			}

// 			self._operations.push(push_op)
// 		}
// 	}
// }
