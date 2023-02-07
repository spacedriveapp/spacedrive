use crate::generator::prelude::*;

use super::sync_id;

/// Generates definitions for a model's `CreateParams` and `CRDTCreateParams` structs
///
/// ## Example
///
/// ```
/// #[derive(Clone)]
/// pub struct CreateParams {
///     pub _params: Vec<SetParam>,
///     pub name: String,
///     pub profile_id: i32
/// }
///
/// #[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
/// pub struct CRDTCreateParams {
///     #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "_")]
///     pub _params: Vec<CRDTSetParam>,
///     #[serde(flatten)]
///     pub _sync_id: SyncID,
///     pub name: String,
///     pub profile_id: Vec<u8>
/// }
/// ```
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
		.filter(|f| !scalar_sync_id_fields.any(|sf| sf.0.name() == f.name()))
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

/// Generates a list of a model's `CreateParams` as function arguments
///
/// ## Example
///
/// ```
/// name: String, profile_id: i32, _params: Vec<SetParam>
/// ```
pub fn args(model: ModelRef, namespace: Option<TokenStream>) -> Vec<TokenStream> {
	let mut required_args = model
		.required_scalar_fields()
		.into_iter()
		.map(|field| {
			let field_name_snake = snake_ident(field.name());

			let typ = match &field.field_type() {
				dml::FieldType::Scalar(_, _, _) => field.type_tokens(),
				dml::FieldType::Enum(e) => {
					let enum_name_pascal = pascal_ident(e);

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

/// Generates a constructor for the `CreateParams` struct
/// that assumes all required fields have been declared beforehand.
///
/// ## Example
///
/// ```
/// CreateParams {
///     name,
///     profile_id,
///     _params
/// }
/// ```
pub fn constructor(model: ModelRef) -> TokenStream {
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

/// Generates a constructor for the CRDTCreateParams struct.
/// Assumes all required fields are in scope.
///
/// ## Example
///
/// ```
/// CRDTCreateParams {
///     _param: {
///         let mut params = vec![];
///
///         for _param in self.set_params._params {
///             params.push(_param.into_crdt(&self.crdt_client).await);
///         }
///
///         params
///     },
///     _sync_id: sync_id.clone(),
///     name: self.set_params.name,
///     profile_id: self
///         .crdt_client
///         .client
///         .profile()
///         .find_unique(crate::prisma::profile::local_id::equals(self.set_params.profile_id))
///         .exec()
///         .await
///         .unwrap()
///         .unwrap()
///         .local_id
/// }
/// ```
pub fn crdt_constructor(model: ModelRef) -> TokenStream {
	let crdt_create_params = model
		.fields()
		.into_iter()
		.filter(|f| {
			f.is_scalar_field()
				&& f.required_on_create()
				&& model
					.scalar_sync_id_fields(&model.datamodel)
					.all(|(sf, _)| sf.name() != f.name())
		})
		.map(|field| {
			let field_name_snake = snake_ident(field.name());

			let value = sync_id::scalar_field_to_crdt(
				field,
				quote!(self.crdt_client.client),
				quote!(self.set_params.#field_name_snake),
			);

			quote!(#field_name_snake: #value)
		});

	quote! {
		CRDTCreateParams {
			_params: {
				let mut params = vec![];

				for _param in self.set_params._params {
					params.push(_param.into_crdt(&self.crdt_client).await);
				}

				params
			},
			_sync_id: sync_id.clone(),
			#(#crdt_create_params,)*
		};
	}
}
