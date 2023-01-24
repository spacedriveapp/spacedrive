use crate::generator::prelude::*;

use super::{create_params, owned, relation, shared};

/// Generates a call to the underlying Prisma client's `create` method for
/// the given model
///
/// ## Example
///
/// ```
/// self
///     .crdt_client
///     .client
///     .user()
///     .create(
///         self.set_params.name.clone(),
///         self.set_params.profile_id.clone),
///         self.set_params._params.clone().into_iter().map(Into::into).collect()
///     )
///     .exec()
///     .await?
/// ```
pub fn prisma_create(model: ModelRef) -> TokenStream {
	let model_name = snake_ident(&model.name);

	let create_args = model
        .fields
        .iter()
        .filter(|f| {
            f.required_on_create()
                && f.as_scalar_field()
                    .map(|sf| !model.scalar_field_has_relation(sf))
                    .unwrap_or(true)
        })
        .map(|field| {
            let field_name_snake = snake_ident(field.name());

            match &field.typ {
                FieldType::Relation {relation_info} => {
                    let relation_model_snake = snake_ident(relation_info.to);

                    if relation_info.fields.len() == 1 {
                        let relation_field_snake = snake_ident(&relation_info.fields[0]);
                        let referenced_field_snake = snake_ident(&relation_info.references[0]);

                        quote!(crate::prisma::#relation_model_snake::#referenced_field_snake::equals(self.set_params.#relation_field_snake.clone()))
                    } else {
                        todo!()
                    }
                },
                _ => quote!(self.set_params.#field_name_snake.clone()),
            }
        });

	quote! {
		self
			.crdt_client
			.client
			.#model_name()
			.create(
				#(#create_args,)*
				self.set_params._params.clone().into_iter().map(Into::into).collect(),
			)
			.exec()
			.await?;
	}
}

/// Generates the definition for a model's `Create` struct
///
/// ## Example
///
/// ```
/// pub struct Create<'a> {
///     crdt_client: &'a super::_prisma::PrismaCRDTClient,
///     set_params: CreateParams,
///     with_params: Vec<crate::prisma::#model_name_snake::WithParam>
/// }
///
/// impl<'a> Create<'a> {
///     pub(super) fn new(
///         crdt_client: &'a super::_prisma::PrismaCRDTClient,
///         set_params: CreateParams,
///         with_params: Vec<crate::prisma::#model_name_snake::WithParam>
///     )
///
///     ..
///
///     pub async fn exec(
///         self
///     ) -> Result<crate::prisma::#model_name_snake::WithParam, crate::prisma::QueryError> {
///         let res = self
///             .crdt_client
///             .client
///             .#model_name_snake()
///             .create(
///                 ..
///             )
///             .exec()
///             .await?;
///
///         ..
///
///         Ok(res)
///     }
/// }
/// ```
pub fn struct_definition(model: ModelRef) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);

	let create_call = prisma_create(model);

	let exec_body = match model.typ {
		ModelType::Owned { .. } => owned::create_exec_body(model),
		ModelType::Shared { .. } => shared::create_exec_body(model),
		ModelType::Relation { .. } => relation::create_exec_body(model),
		// SAFETY: Local models don't have method overrides
		ModelType::Local { .. } => unreachable!(),
	};

	quote! {
		pub struct Create<'a> {
			crdt_client: &'a super::_prisma::PrismaCRDTClient,
			set_params: CreateParams,
			with_params: Vec<crate::prisma::#model_name_snake::WithParam>,
		}

		impl<'a> Create<'a> {
			pub(super) fn new(
				crdt_client: &'a super::_prisma::PrismaCRDTClient,
				set_params: CreateParams,
				with_params: Vec<crate::prisma::#model_name_snake::WithParam>,
			) -> Self {
				Self {
					crdt_client,
					set_params,
					with_params,
				}
			}

			pub fn with(mut self, param: impl Into<crate::prisma::#model_name_snake::WithParam>) -> Self {
				self.with_params.push(param.into());
				self
			}

			pub async fn exec(
				self,
			) -> Result<crate::prisma::#model_name_snake::Data, crate::prisma::QueryError> {
				let res = #create_call;

				#exec_body

				Ok(res)
			}
		}
	}
}

/// Generates a model's `Actions::create` method
///
/// ## Example
///
/// ```
/// pub fn create(self, name: String, profile_id: i32, _params: Vec<SetParam>) -> Create<'a> {
///     Create::new(
///         self.client,
///         CreateParams { .. },
///         vec![]
///     )
/// }
/// ```
pub fn action_method(model: ModelRef) -> TokenStream {
	let args = create_params::args(model, Some(quote!(super)));

	let create_params_constructor = create_params::constructor(model);

	quote! {
		pub fn create(self, #(#args),*) -> Create<'a> {
			Create::new(
				self.client,
				#create_params_constructor,
				vec![]
			)
		}
	}
}
