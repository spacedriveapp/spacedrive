use crate::generator::prelude::*;

pub fn definition(model: ModelRef) -> TokenStream {
	let required_scalar_fields = model.required_scalar_fields();

	let required_create_params = required_scalar_fields.iter().map(|field| {
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

	let mut scalar_sync_id_fields = model.scalar_sync_id_fields(&model.datamodel);

	let required_crdt_create_params = required_scalar_fields
		.iter()
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

pub fn args(model: ModelRef, namespace: Option<TokenStream>) -> Vec<TokenStream> {
	let model_name_snake = snake_ident(&model.name);

	let mut required_args = model
		.required_scalar_fields()
		.into_iter()
		.map(|field| {
			let field_name_snake = snake_ident(field.name());

			let typ = match &field.field_type() {
				dml::FieldType::Scalar(_, _, _) => field.type_tokens(),
				dml::FieldType::Enum(e) => {
					let enum_name_pascal = pascal_ident(&e);

					quote!(#(#namespace::)super::#enum_name_pascal)
				}
				_ => unreachable!(),
			};

			quote!(#field_name_snake: #typ)
		})
		.collect::<Vec<_>>();

	required_args.push(quote!(_params: Vec<SetParam>));

	required_args
}

/// Generates a constructor for the CreateParams struct
/// that assumes all required fields have been declared beforehand.
pub fn shorthand_constructor(model: ModelRef) -> TokenStream {
	let required_args = model
		.required_scalar_fields()
		.into_iter()
		.map(|field| snake_ident(field.name()));

	quote! {
        CreateParams {
            #(#required_args,)*
            _params
        }
	}
}
