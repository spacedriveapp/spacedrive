use crate::generator::prelude::*;

use super::sync_id::scalar_field_to_crdt;

/// Generates the struct definition for a relation's key
pub fn relation_key_definition(field: FieldRef, struct_name: TokenStream) -> TokenStream {
	let fields = match &field.typ {
		FieldType::Relation { relation_info } => relation_info.fields.iter().map(|rel_field| {
			let field_name_snake = snake_ident(rel_field);
			let field = field
				.model
				.field(rel_field)
				.unwrap_or_else(|| panic!("Model {} has no field {}", field.model.name, rel_field));

			let field_type = field.crdt_type_tokens(&field.datamodel);

			quote!(#field_name_snake: #field_type)
		}),
		_ => unreachable!(),
	};

	quote! {
		#[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
		pub struct #struct_name {
			#(pub #fields),*
		}
	}
}

/// Generates a constructor for a relation's key
pub fn relation_key_constructor(field: FieldRef, struct_name: TokenStream) -> TokenStream {
	let key_args = match &field.typ {
		FieldType::Relation { relation_info } => relation_info.fields.iter().map(|rel_field| {
			let field_name_snake = snake_ident(rel_field);
			let field = field.model.field(rel_field).unwrap();

			let value = scalar_field_to_crdt(
				field,
				quote!(self.crdt_client.client),
				quote!(self.set_params.#field_name_snake),
			);

			quote!(#field_name_snake: #value)
		}),
		_ => unreachable!(), // Item & group must be relations
	};

	quote! {
		#struct_name {
			#(#key_args),*
		}
	}
}

/// Generates the body for a relation model's `Create::exec` function
///
/// ## Example
///
/// ```
/// let relation_item = RelationItem { .. };
///
/// let relation_group = RelationGroup { .. };
///
/// self
///     .crdt_client
///     ._create_operation(::prisma_crdt::CRDTOperationType::relation(
///         #model_name_str,
///         ::prisma_crdt::objectify(relation_item),
///         ::prisma_crdt::objectify(relation_group),
///         ::prisma_crdt::RelationOperationData::create()
///     ))
///     .await;
/// ```
pub fn create_exec_body(model: ModelRef) -> TokenStream {
	let model_name_str = &model.name;

	let (relation_item_block, relation_group_block) = match &model.typ {
		ModelType::Relation { item, group } => {
			let relation_item_block = relation_key_constructor(
				model.field(item.at_index(0).unwrap()).unwrap(),
				quote!(RelationItem),
			);
			let relation_group_block = relation_key_constructor(
				model.field(group.at_index(0).unwrap()).unwrap(),
				quote!(RelationGroup),
			);

			(relation_item_block, relation_group_block)
		}
		_ => unreachable!(),
	};

	quote! {
		let relation_item = #relation_item_block;

		let relation_group = #relation_group_block;

		self
			.crdt_client
			._create_operation(::prisma_crdt::CRDTOperationType::relation(
				#model_name_str,
				::prisma_crdt::objectify(relation_item),
				::prisma_crdt::objectify(relation_group),
				::prisma_crdt::RelationOperationData::create()
			))
			.await;
	}
}
