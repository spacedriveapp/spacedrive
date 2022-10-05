use crate::generator::prelude::*;

use super::create;

/// Generates struct definition for a model's `Actions` struct
///
/// ## Example
///
/// ```
/// pub struct Actions<'a> {
///     client: &'a super::_prisma::PrismaCRDTClient
/// }
///
/// impl<'a> Actions<'a> {
///     pub(super) fn new(client: &'a super::_prisma::PrismaCRDTClient) -> Self {
///         Self { client }
///     }
///
///     pub fn create(..) {
///         ..
///     }
///
///     pub fn find_unique(
///         self,
///         param: crate::prisma::#model::UniqueWhereParam
///     ) -> crate::prisma::#model::FindUnique<'a> {
///         self.client.client.#model().find_unique(param)
///     }
///
///     pub fn find_many(
///         self,
///         params: Vec<crate::prisma::#model::WhereParam>
///     ) -> crate::prisma::#model::FindMany<'a> {
///         self.client.client.#model().find_many(params)
///     }
/// }
/// ```
pub fn definition(model: ModelRef) -> TokenStream {
	let name = snake_ident(&model.name);

	let create_fn = create::action_method(model);

	quote! {
		pub struct Actions<'a> {
			client: &'a super::_prisma::PrismaCRDTClient,
		}

		impl<'a> Actions<'a> {
			pub(super) fn new(client: &'a super::_prisma::PrismaCRDTClient) -> Self {
				Self { client }
			}

			#create_fn

			pub fn find_unique(
				self,
				param: crate::prisma::#name::UniqueWhereParam,
			) -> crate::prisma::#name::FindUnique<'a> {
				self.client.client.#name().find_unique(param)
			}

			pub fn find_many(
				self,
				params: Vec<crate::prisma::#name::WhereParam>,
			) -> crate::prisma::#name::FindMany<'a> {
				self.client.client.#name().find_many(params)
			}

			pub fn update(
				self,
				_where: crate::prisma::#name::UniqueWhereParam,
				set_params: Vec<SetParam>,
			) -> Update<'a> {
				Update {
					client: self.client,
					where_param: _where,
					set_params,
				}
			}

			// pub fn delete(self, param: crate::prisma::#name::UniqueWhereParam) -> Delete<'a> {
			// 	Delete {
			// 		client: self.client,
			// 		r#where: param,
			// 		with_params: vec![],
			// 	}
			// }
		}
	}
}
