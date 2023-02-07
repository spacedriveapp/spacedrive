use std::ops::Deref;

use crate::prelude::*;

#[derive(Debug)]
pub struct Field<'a> {
	pub prisma: &'a dml::Field,
	pub model: &'a str,
	pub typ: FieldType<'a>,
}

impl<'a> Field<'a> {
	pub fn new(field: &'a dml::Field, model: &'a str, datamodel: &'a dml::Datamodel) -> Field<'a> {
		let typ = FieldType::new(field, model, datamodel);

		Self {
			prisma: field,
			model,
			typ,
		}
	}

	/// Returns the token representation of the field's type,
	/// accounting for a sync ID reference if it is a field
	/// of a relation
	pub fn crdt_type_tokens(&self, datamodel: &Datamodel) -> TokenStream {
		let relation_field_info = match &self.typ {
			FieldType::Scalar {
				relation_field_info,
			} => relation_field_info,
			_ => unreachable!("Cannot get CRDT type for non-scalar field"),
		};

		match relation_field_info.as_ref() {
			Some(relation_field_info) => {
				let relation_model = datamodel
					.model(relation_field_info.referenced_model)
					.unwrap();

				let sync_id_field =
					relation_model.sync_id_for_pk(relation_field_info.referenced_field);

				match sync_id_field {
					Some(field) => {
						let relation_field_type = field.field_type().to_tokens();

						match self.arity() {
							dml::FieldArity::Required => relation_field_type,
							dml::FieldArity::Optional => quote!(Option<#relation_field_type>),
							dml::FieldArity::List => quote!(Vec<#relation_field_type>),
						}
					}
					None => self.type_tokens(),
				}
			}
			None => datamodel
				.model(self.model)
				.unwrap()
				.sync_id_for_pk(self.name())
				.unwrap_or(self)
				.type_tokens(),
		}
	}
}

impl<'a> Deref for Field<'a> {
	type Target = dml::Field;
	fn deref(&self) -> &Self::Target {
		self.prisma
	}
}

#[derive(Debug)]
pub enum FieldType<'a> {
	Scalar {
		/// The relation field that this scalar field is a part of.
		relation_field_info: Option<RelationFieldInfo<'a>>,
	},
	Relation {
		relation_info: RelationInfo<'a>,
	},
}

impl<'a> FieldType<'a> {
	fn new(field: &'a dml::Field, model: &str, datamodel: &'a dml::Datamodel) -> Self {
		match field.field_type() {
			dml::FieldType::Scalar(_, _, _) => FieldType::Scalar {
				relation_field_info: {
					datamodel
						.find_model(model)
						.unwrap()
						.fields()
						.find_map(|relation_field| {
							relation_field
								.as_relation_field()
								.and_then(|relation_field_data| {
									relation_field_data
										.relation_info
										.fields
										.iter()
										.position(|rf_name| rf_name == field.name())
										.map(|pos| (relation_field_data, pos))
								})
								.and_then(|(relation_field_data, i)| {
									datamodel
										.models()
										.find(|relation_model| {
											relation_model.name
												== relation_field_data.relation_info.to
										})
										.and_then(|relation_model| {
											relation_model
												.fields()
												.find(|referenced_field| {
													referenced_field.name()
														== relation_field_data
															.relation_info
															.references[i]
												})
												.map(|f| (relation_model, f))
										})
								})
								.map(|(ref_model, ref_field)| {
									(relation_field.name(), &ref_model.name, ref_field.name())
								})
						})
						.map(|(rel, ref_model, ref_field)| {
							RelationFieldInfo::new(rel, ref_model, ref_field)
						})
				},
			},
			dml::FieldType::Relation(_) => FieldType::Relation {
				relation_info: {
					field
						.as_relation_field()
						.filter(|rf| !OPERATION_MODELS.contains(&rf.relation_info.to.as_str()))
						.map(|rf| {
							RelationInfo::new(
								&rf.relation_info.to,
								&rf.relation_info.fields,
								&rf.relation_info.references,
							)
						})
						.unwrap()
				},
			},
			t => unimplemented!("Unsupported field type: {:?}", t),
		}
	}
}

#[derive(Debug)]
pub struct RelationFieldInfo<'a> {
	/// Field on the same model that represents the relation
	pub relation: &'a str,
	pub referenced_model: &'a str,
	/// Scalar field on the referenced model that matches the scalar on the same model
	pub referenced_field: &'a str,
}

impl<'a> RelationFieldInfo<'a> {
	pub fn new(relation: &'a str, referenced_model: &'a str, referenced_field: &'a str) -> Self {
		Self {
			relation,
			referenced_model,
			referenced_field,
		}
	}
}

#[derive(Debug)]
pub struct RelationInfo<'a> {
	pub to: &'a str,
	pub fields: &'a Vec<String>,
	pub references: &'a Vec<String>,
}

impl<'a> RelationInfo<'a> {
	pub fn new(to: &'a str, fields: &'a Vec<String>, references: &'a Vec<String>) -> Self {
		Self {
			to,
			fields,
			references,
		}
	}
}
