use crate::generator::prelude::*;

use super::sync_id::scalar_field_to_crdt;

pub fn relation_key_definition(field: FieldRef, struct_name: TokenStream) -> TokenStream {
	let fields = match &field.typ {
		FieldType::Relation { relation_info } => relation_info.fields.iter().map(|rel_field| {
			let field_name_snake = snake_ident(rel_field);
			let field = field.model.field(rel_field).expect(&format!(
				"Model {} has no field {}",
				field.model.name, rel_field
			));

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

pub fn relation_key_constructor(field: FieldRef, struct_name: TokenStream) -> TokenStream {
	let key_args = match &field.typ {
		FieldType::Relation { relation_info } => relation_info.fields.iter().map(|rel_field| {
			let field_name_snake = snake_ident(rel_field);
			let field = field.model.field(rel_field).unwrap();

			let value = scalar_field_to_crdt(
				field,
				quote!(self.client.client),
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
