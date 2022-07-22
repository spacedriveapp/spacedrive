use crate::generator::prelude::*;

use super::sync_id::ScalarFieldToCRDT;

pub struct RelationKeyDefinition<'a> {
	field: &'a Field<'a>,
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
	struct_name: TokenStream,
}

impl<'a> RelationKeyDefinition<'a> {
	pub fn new(
		field: &'a Field<'a>,
		model: &'a Model<'a>,
		datamodel: &'a Datamodel<'a>,
		struct_name: TokenStream,
	) -> Self {
		Self {
			field,
			model,
			datamodel,
			struct_name,
		}
	}
}

impl ToTokens for RelationKeyDefinition<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let fields = match &self.field.typ {
			FieldType::Relation { relation_info } => relation_info.fields.iter().map(|field| {
				let field_name_snake = snake_ident(field);
				let field = self
					.model
					.field(field)
					.expect(&format!("Model {} has no field {}", self.model.name, field));

				let field_type = field.crdt_type_tokens(self.datamodel);

				quote!(#field_name_snake: #field_type)
			}),
			_ => unreachable!(),
		};

		let struct_name = &self.struct_name;

		tokens.extend(quote! {
			#[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
			pub struct #struct_name {
				#(pub #fields),*
			}
		})
	}
}

pub struct RelationKeyConstructor<'a> {
	field: &'a Field<'a>,
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
	struct_name: TokenStream,
}

impl<'a> RelationKeyConstructor<'a> {
	pub fn new(
		field: &'a Field<'a>,
		model: &'a Model<'a>,
		datamodel: &'a Datamodel<'a>,
		struct_name: TokenStream,
	) -> Self {
		Self {
			field,
			model,
			datamodel,
			struct_name,
		}
	}
}

impl ToTokens for RelationKeyConstructor<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let key_args = match &self.field.typ {
			FieldType::Relation { relation_info } => relation_info.fields.iter().map(|field| {
				let field_name_snake = snake_ident(field);
				let field = self.model.field(field).unwrap();

				let value = ScalarFieldToCRDT::new(
					field,
					self.model,
					self.datamodel,
					quote!(self.client.client),
					quote!(self.set_params.#field_name_snake),
				);

				quote!(#field_name_snake: #value)
			}),
			_ => unreachable!(), // Item & group must be relations
		};

		let struct_name = &self.struct_name;

		tokens.extend(quote! {
			#struct_name {
				#(#key_args),*
			}
		});
	}
}
