use std::{ops::Deref, str::FromStr};

use crate::attribute::{Attribute, AttributeFieldValue};
use crate::prelude::*;

#[derive(Debug)]
pub struct Model<'a> {
	pub prisma: &'a dml::Model,
	pub typ: ModelType,
	pub fields: Vec<Field<'a>>,
}

impl<'a> Model<'a> {
	pub fn new(model: &'a dml::Model, datamodel: &'a dml::Datamodel) -> Result<Self, String> {
		let crdt_attribute = model
			.documentation
			.as_ref()
			.map(Attribute::parse)
			.map(Result::unwrap)
			.unwrap();

		let fields = model
			.fields()
			.filter(|f| {
				f.as_relation_field()
					.filter(|rf| OPERATION_MODELS.contains(&rf.relation_info.to.as_str()))
					.is_none()
			})
			.map(|f| Field::new(f, &model.name, datamodel))
			.collect::<Vec<_>>();

		let typ = ModelType::from_attribute(&crdt_attribute, &fields, model)?;

		let model = Self {
			prisma: model,
			typ,
			fields,
		};

		Ok(model)
	}

	pub fn field(&self, name: &str) -> Option<&Field<'a>> {
		self.fields.iter().find(|f| f.name() == name)
	}

	pub fn is_sync_id(&self, field: &str) -> bool {
		match &self.typ {
			ModelType::Local { id } => id.is_sync_id(field),
			ModelType::Owned { id, .. } => id.is_sync_id(field),
			ModelType::Shared { id, .. } => id.is_sync_id(field),
			ModelType::Relation { item, group } => {
				item.is_sync_id(field) || group.is_sync_id(field)
			}
		}
	}

	pub fn is_pk(&self, field: &str) -> bool {
		self.primary_key
			.as_ref()
			.unwrap()
			.fields
			.iter()
			.any(|f| f.name == field)
	}

	pub fn sync_id_for_pk(&self, primary_key: &str) -> Option<&Field<'a>> {
		let pk_index = self
			.primary_key
			.as_ref()
			.unwrap()
			.fields
			.iter()
			.position(|f| f.name == primary_key);

		pk_index
			.and_then(|pk_index| match &self.typ {
				ModelType::Local { id } => id.at_index(pk_index),
				ModelType::Owned { id, .. } => id.at_index(pk_index),
				ModelType::Shared { id, .. } => id.at_index(pk_index),
				ModelType::Relation { item, group } => {
					item.at_index(0).or_else(|| group.at_index(0))
				}
			})
			.and_then(|f| self.field(f))
	}

	/// Gets the scalar sync id fields for a model, along with the (possibly) foreign field
	/// that their types should be resolved from.
	///
	/// For example, a scalar field will have no difference between the first and second element.
	/// A relation, however, will result in the first element being the model's scalar field,
	/// and the second element being the foreign scalar field. It is important to note that these foreign
	/// fields could be primary keys that map to sync ids, and this should be checked.
	pub fn scalar_sync_id_fields(
		&'a self,
		datamodel: &'a Datamodel,
	) -> impl Iterator<Item = (&'a Field, &'a Field)> {
		self.fields
			.iter()
			.filter(|f| self.is_sync_id(f.name()))
			.flat_map(|field| match &field.typ {
				FieldType::Scalar { .. } => {
					vec![(field, field)]
				}
				FieldType::Relation { relation_info } => relation_info
					.fields
					.iter()
					.enumerate()
					.map(|(i, field)| {
						let relation_model = datamodel.model(relation_info.to).unwrap();
						// Scalar field on the relation model. Could be a local id,
						// so crdt type must be used
						let referenced_field =
							relation_model.field(&relation_info.references[i]).unwrap();

						(self.field(field).unwrap(), referenced_field)
					})
					.collect(),
			})
	}
}

impl<'a> Deref for Model<'a> {
	type Target = dml::Model;
	fn deref(&self) -> &Self::Target {
		self.prisma
	}
}

#[derive(Debug)]
pub enum ModelType {
	Local {
		id: SyncIDMapping,
	},
	Owned {
		owner: String,
		id: SyncIDMapping,
	},
	Shared {
		id: SyncIDMapping,
		create: SharedCreateType,
	},
	Relation {
		item: SyncIDMapping,
		group: SyncIDMapping,
	},
}

impl ModelType {
	pub fn from_attribute(
		attribute: &Attribute,
		fields: &[Field],
		model: &dml::Model,
	) -> Result<Self, String> {
		let ret = match attribute.name {
			"local" => {
				let id = SyncIDMapping::from_attribute(attribute.field("id"), fields, model)?;

				ModelType::Local { id }
			}
			"owned" => {
				let id = SyncIDMapping::from_attribute(attribute.field("id"), fields, model)?;

				let owner = attribute
					.field("owner")
					.ok_or_else(|| "Missing owner field".to_string())
					.map(|owner| owner.as_single().expect("Owner field must be a string"))
					.and_then(|owner| {
						fields
							.iter()
							.find(|f| f.name() == owner)
							.map(|f| f.name().to_string())
							.ok_or(format!("Unknown owner field {}", owner))
					})?;

				ModelType::Owned { id, owner }
			}
			"shared" => {
				let id = SyncIDMapping::from_attribute(attribute.field("id"), fields, model)?;

				let create = attribute
					.field("create")
					.map(|create| create.as_single().expect("create field must be a string"))
					.map(SharedCreateType::from_str)
					.unwrap_or(Ok(SharedCreateType::Unique))?;

				ModelType::Shared { id, create }
			}
			"relation" => {
				let item = SyncIDMapping::from_attribute(
					Some(
						attribute
							.field("item")
							.expect("@relation attribute missing `item` field"),
					),
					fields,
					model,
				)?;
				let group = SyncIDMapping::from_attribute(
					Some(
						attribute
							.field("group")
							.expect("@relation attribute missing `group` field"),
					),
					fields,
					model,
				)?;

				ModelType::Relation { item, group }
			}
			name => Err(format!("Invalid attribute type {name}"))?,
		};

		Ok(ret)
	}
}

#[derive(Debug)]
pub enum SyncIDMapping {
	Single(String),
	Compound(Vec<String>),
}

impl SyncIDMapping {
	pub fn from_attribute(
		attr_value: Option<&AttributeFieldValue>,
		fields: &[Field],
		model: &dml::Model,
	) -> Result<Self, String> {
		let primary_key = model
			.primary_key
			.as_ref()
			.ok_or(format!("Model {} has no primary key", model.name))?;

		attr_value
			.map(|attr_value| match attr_value {
				AttributeFieldValue::Single(field) => {
					fields
						.iter()
						.find(|f| f.name() == *field)
						.ok_or(format!("Unknown field {}", field))?;

					Ok(SyncIDMapping::Single(field.to_string()))
				}
				AttributeFieldValue::List(field_list) => {
					if primary_key.fields.len() != field_list.len() {
						return Err(format!(
							"Sync ID for model {} has inconsistent number of fields",
							model.name,
						));
					}

					field_list
						.iter()
						.map(|name| {
							fields
								.iter()
								.find(|f| f.name() == *name)
								.map(|f| f.name().to_string())
						})
						.collect::<Option<_>>()
						.map(SyncIDMapping::Compound)
						.ok_or(format!("Invalid sync ID for model {}", model.name))
				}
			})
			.unwrap_or_else(|| {
				Ok(match primary_key.fields.len() {
					1 => SyncIDMapping::Single(primary_key.fields[0].name.to_string()),
					_ => SyncIDMapping::Compound(
						primary_key
							.fields
							.iter()
							.map(|f| f.name.to_string())
							.collect(),
					),
				})
			})
	}

	pub fn is_sync_id(&self, field: &str) -> bool {
		match self {
			Self::Single(v) => field == v,
			Self::Compound(mappings) => mappings.iter().any(|v| field == v),
		}
	}

	pub fn at_index(&self, i: usize) -> Option<&str> {
		match self {
			Self::Single(v) => Some(v),
			Self::Compound(mappings) => mappings.get(i).map(|v| v.as_str()),
		}
	}
}

#[derive(Debug)]
pub enum SharedCreateType {
	Unique,
	Atomic,
}

impl FromStr for SharedCreateType {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let ret = match s {
			"Unique" => SharedCreateType::Unique,
			"Atomic" => SharedCreateType::Atomic,
			s => Err(format!("Invalid create type {}", s))?,
		};

		Ok(ret)
	}
}
