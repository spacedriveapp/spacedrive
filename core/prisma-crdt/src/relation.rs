use serde::{Deserialize, Serialize};
use serde_json::{Value, Map};

use crate::Id;

/// An operation on a many relation CRDT.
/// Many relations are identified by their `relation` (db table),
/// `relation_item` (subject record) and `relation_group` (group record).
///
/// Many relations represent a Many to Many (M2M) relation between two records,
/// where data about the relation itself is stored in a separate table.
///
/// **NOTE**: This does not include M2M relations where the item can exist in a group multiple times.
///
/// In contrast to shared records, many relations are identified by the records they relate,
/// and do not have their own unique ID.
///
/// ## Create
/// Creating a many relation does not allow for setting data, only for indicating that the relation exists.
/// Setting data can be done with subsequent Update operations. This is enforced as if multiple nodes attempt
/// to create the same relation, multiple relations should not be created - hence the lack of unique IDs for many relations.
///
/// ## Update
/// Updates to many relations are done on a per-field basis, in the same way as shared records.
///
/// ## Delete
/// Deleting many relations use the operation's `relation`, `relation_item` and `relation_group` to identify the relation and delete it.
#[derive(Serialize, Deserialize, Clone)]
pub struct RelationOperation {
	#[serde(rename = "r")]
	pub relation: String,
	#[serde(rename = "ri")]
	pub relation_item: Map<String, Value>,
	#[serde(rename = "rg")]
	pub relation_group: Map<String, Value>,
	#[serde(rename = "d")]
	pub data: RelationOperationData,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum RelationOperationData {
	#[serde(rename = "c")]
	Create,
	#[serde(rename = "u")]
	Update {
		#[serde(rename = "f")]
		field: String,
		#[serde(rename = "v'")]
		value: Value,
	},
	#[serde(rename = "d")]
	Delete,
}

impl RelationOperation {
	pub fn new(
		relation: String,
		relation_item: Map<String, Value>,
		relation_group: Map<String, Value>,
		data: RelationOperationData,
	) -> Self {
		Self {
			relation_item,
			relation_group,
			relation,
			data,
		}
	}
}

impl RelationOperationData {
	pub fn create() -> Self {
		Self::Create
	}

	pub fn update(field: String, value: Value) -> Self {
		Self::Update { field, value }
	}

	pub fn delete() -> Self {
		Self::Delete
	}
}
