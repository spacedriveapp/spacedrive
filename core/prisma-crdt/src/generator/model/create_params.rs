use crate::generator::prelude::*;

pub fn definition(model: ModelRef) -> TokenStream {
	let required_scalar_fields = model
		.fields()
        .into_iter()
		.filter(|field| field.is_scalar_field() && field.required_on_create());

	let mut scalar_sync_id_fields = model.scalar_sync_id_fields(&model.datamodel);

	let required_create_params = required_scalar_fields.clone().map(|field| {
		let field_name_snake = snake_ident(field.name());

		let field_type = match field.field_type() {
			dml::FieldType::Scalar(_, _, _) => field.type_tokens(),
			dml::FieldType::Enum(e) => {
				let enum_name_pascal = pascal_ident(&e);

				quote!(super::#enum_name_pascal)
			}
			_ => todo!(),
		};

		quote!(#field_name_snake: #field_type)
	});

	let required_crdt_create_params = required_scalar_fields
		.clone()
		.filter(|f| {
			scalar_sync_id_fields
				.find(|sf| sf.0.name() == f.name())
				.is_none()
		})
		.map(|field| {
			let field_type = field.crdt_type_tokens(&model.datamodel);
			let field_name_snake = snake_ident(field.name());

			quote!(#field_name_snake: #field_type)
		});

	quote! {
		#[derive(Clone)]
		pub struct CreateParams {
			pub _params: Vec<SetParam>,
			#(pub #required_create_params),*
		}

		#[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
		pub struct CRDTCreateParams {
			#[serde(default, skip_serializing_if = "Vec::is_empty", rename = "_")]
			pub _params: Vec<CRDTSetParam>,
			#[serde(flatten)]
			pub _sync_id: SyncID,
			#(pub #required_crdt_create_params),*
		}
	}
}
